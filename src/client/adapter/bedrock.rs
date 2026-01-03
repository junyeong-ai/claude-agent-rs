//! AWS Bedrock adapter using InvokeModel API (Messages API compatible).
//!
//! Uses the official Anthropic Messages API format with SigV4 signing.
//! Supports global and regional endpoints as documented at:
//! <https://platform.claude.com/docs/en/build-with-claude/claude-on-amazon-bedrock>

use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_credential_types::provider::ProvideCredentials;
use aws_sigv4::http_request::{SignableBody, SignableRequest, SigningSettings, sign};
use aws_sigv4::sign::v4::SigningParams;
use aws_smithy_runtime_api::client::identity::Identity;

use super::base::RequestExecutor;
use super::config::{BetaFeature, ProviderConfig};
use super::request::{add_beta_features, build_messages_body};
use super::token_cache::{AwsCredentialsCache, CachedAwsCredentials, new_aws_credentials_cache};
use super::traits::ProviderAdapter;
use crate::client::messages::CreateMessageRequest;
use crate::types::ApiResponse;
use crate::{Error, Result};

const ANTHROPIC_VERSION: &str = "bedrock-2023-05-31";

/// Bedrock adapter using InvokeModel API with Messages API format.
#[derive(Debug)]
pub struct BedrockAdapter {
    config: ProviderConfig,
    region: String,
    small_model_region: Option<String>,
    use_global_endpoint: bool,
    enable_1m_context: bool,
    auth: BedrockAuth,
    credentials_cache: AwsCredentialsCache,
}

#[derive(Debug)]
enum BedrockAuth {
    SigV4(Arc<dyn ProvideCredentials>),
    BearerToken(String),
}

impl BedrockAdapter {
    /// Create adapter from environment variables.
    pub async fn from_env(config: ProviderConfig) -> Result<Self> {
        let bedrock_config = crate::config::BedrockConfig::from_env();
        Self::from_config(config, bedrock_config).await
    }

    /// Create adapter from explicit configuration.
    pub async fn from_config(
        config: ProviderConfig,
        bedrock: crate::config::BedrockConfig,
    ) -> Result<Self> {
        let region = bedrock.region.unwrap_or_else(|| "us-east-1".into());

        let auth = if let Some(token) = bedrock.bearer_token {
            BedrockAuth::BearerToken(token)
        } else {
            let aws_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
            let credentials = aws_config
                .credentials_provider()
                .ok_or_else(|| Error::auth("No AWS credentials found"))?;
            BedrockAuth::SigV4(Arc::from(credentials))
        };

        Ok(Self {
            config,
            region,
            small_model_region: bedrock.small_model_region,
            use_global_endpoint: bedrock.use_global_endpoint,
            enable_1m_context: bedrock.enable_1m_context,
            auth,
            credentials_cache: new_aws_credentials_cache(),
        })
    }

    /// Set the AWS region.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Set a separate region for small/fast models (e.g., Haiku).
    pub fn with_small_model_region(mut self, region: impl Into<String>) -> Self {
        self.small_model_region = Some(region.into());
        self
    }

    /// Enable or disable global endpoint (default: true).
    pub fn with_global_endpoint(mut self, enable: bool) -> Self {
        self.use_global_endpoint = enable;
        self
    }

    /// Enable 1M context window beta feature.
    pub fn with_1m_context(mut self, enable: bool) -> Self {
        self.enable_1m_context = enable;
        self
    }

    /// Set bearer token authentication.
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth = BedrockAuth::BearerToken(token.into());
        self
    }

    /// Get the effective region for a given model.
    fn region_for_model(&self, model: &str) -> &str {
        if let Some(ref small_region) = self.small_model_region
            && model.contains("haiku")
        {
            return small_region;
        }
        &self.region
    }

    fn build_invoke_url(&self, model: &str, stream: bool) -> String {
        let region = self.region_for_model(model);
        let endpoint = if stream {
            "invoke-with-response-stream"
        } else {
            "invoke"
        };
        let encoded_model = urlencoding::encode(model);

        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/{}",
            region, encoded_model, endpoint
        )
    }

    /// Build Messages API compatible request body.
    fn build_request_body(&self, request: &CreateMessageRequest) -> serde_json::Value {
        let mut body = build_messages_body(
            request,
            Some(ANTHROPIC_VERSION),
            self.config.thinking_budget,
        );

        // Bedrock doesn't include model in body (it's in the URL)
        if let Some(obj) = body.as_object_mut() {
            obj.remove("model");
        }

        if self.enable_1m_context {
            add_beta_features(&mut body, &[BetaFeature::Context1M.header_value()]);
        }

        body
    }

    /// Get cached or fresh AWS credentials.
    async fn get_credentials(&self) -> Result<CachedAwsCredentials> {
        let provider = match &self.auth {
            BedrockAuth::SigV4(p) => p,
            BedrockAuth::BearerToken(_) => {
                return Err(Error::auth("Bearer token mode does not use credentials"));
            }
        };

        {
            let cache = self.credentials_cache.read().await;
            if let Some(ref creds) = *cache
                && !creds.is_expired()
            {
                return Ok(creds.clone());
            }
        }

        let creds = provider
            .provide_credentials()
            .await
            .map_err(|e| Error::auth(e.to_string()))?;

        let cached = CachedAwsCredentials::new(
            creds.access_key_id().to_string(),
            creds.secret_access_key().to_string(),
            creds.session_token().map(|s| s.to_string()),
            creds.expiry(),
        );

        *self.credentials_cache.write().await = Some(cached.clone());
        Ok(cached)
    }

    /// Get authorization headers for a request.
    async fn get_auth_headers(
        &self,
        method: &str,
        url: &str,
        body: &[u8],
        region: &str,
    ) -> Result<Vec<(String, String)>> {
        match &self.auth {
            BedrockAuth::BearerToken(token) => {
                Ok(vec![("Authorization".into(), format!("Bearer {}", token))])
            }
            BedrockAuth::SigV4(_) => self.sign_request(method, url, body, region).await,
        }
    }

    /// Sign request with SigV4.
    async fn sign_request(
        &self,
        method: &str,
        url: &str,
        body: &[u8],
        region: &str,
    ) -> Result<Vec<(String, String)>> {
        let creds = self.get_credentials().await?;

        let aws_creds = aws_credential_types::Credentials::new(
            &creds.access_key_id,
            &creds.secret_access_key,
            creds.session_token.clone(),
            creds.expiry(),
            "bedrock-adapter",
        );

        let identity = Identity::new(aws_creds, creds.expiry());

        let signing_params = SigningParams::builder()
            .identity(&identity)
            .region(region)
            .name("bedrock")
            .time(SystemTime::now())
            .settings(SigningSettings::default())
            .build()
            .map_err(|e| Error::auth(e.to_string()))?;

        let signable_request = SignableRequest::new(
            method,
            url,
            std::iter::empty::<(&str, &str)>(),
            SignableBody::Bytes(body),
        )
        .map_err(|e| Error::auth(e.to_string()))?;

        let (signing_instructions, _) = sign(signable_request, &signing_params.into())
            .map_err(|e| Error::auth(e.to_string()))?
            .into_parts();

        Ok(signing_instructions
            .headers()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect())
    }

    async fn execute_request(
        &self,
        http: &reqwest::Client,
        url: &str,
        body_bytes: Vec<u8>,
        region: &str,
    ) -> Result<reqwest::Response> {
        let headers = self
            .get_auth_headers("POST", url, &body_bytes, region)
            .await?;
        RequestExecutor::post_bytes(http, url, body_bytes, headers).await
    }
}

#[async_trait]
impl ProviderAdapter for BedrockAdapter {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn name(&self) -> &'static str {
        "bedrock"
    }

    async fn build_url(&self, model: &str, stream: bool) -> String {
        self.build_invoke_url(model, stream)
    }

    async fn transform_request(&self, request: CreateMessageRequest) -> serde_json::Value {
        self.build_request_body(&request)
    }

    fn transform_response(&self, response: serde_json::Value) -> Result<ApiResponse> {
        // InvokeModel returns Messages API format directly
        serde_json::from_value(response).map_err(|e| Error::Parse(e.to_string()))
    }

    async fn send(
        &self,
        http: &reqwest::Client,
        request: CreateMessageRequest,
    ) -> Result<ApiResponse> {
        let model = request.model.clone();
        let region = self.region_for_model(&model);
        let url = self.build_invoke_url(&model, false);
        let body = self.build_request_body(&request);
        let body_bytes = serde_json::to_vec(&body)?;

        let response = self.execute_request(http, &url, body_bytes, region).await?;
        let json: serde_json::Value = response.json().await?;
        self.transform_response(json)
    }

    async fn send_stream(
        &self,
        http: &reqwest::Client,
        mut request: CreateMessageRequest,
    ) -> Result<reqwest::Response> {
        request.stream = Some(true);
        let model = request.model.clone();
        let region = self.region_for_model(&model);
        let url = self.build_invoke_url(&model, true);
        let body = self.build_request_body(&request);
        let body_bytes = serde_json::to_vec(&body)?;

        self.execute_request(http, &url, body_bytes, region).await
    }

    async fn refresh_credentials(&self) -> Result<()> {
        if matches!(self.auth, BedrockAuth::SigV4(_)) {
            *self.credentials_cache.write().await = None;
            self.get_credentials().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::adapter::ModelConfig;
    use serde_json::json;

    #[test]
    fn test_url_encoding() {
        let model = "global.anthropic.claude-sonnet-4-5-20250929-v1:0";
        let encoded = urlencoding::encode(model);
        assert!(encoded.contains("%3A"));
        assert!(encoded.contains("global.anthropic"));
    }

    #[test]
    fn test_invoke_url_format() {
        let model = "global.anthropic.claude-sonnet-4-5-20250929-v1:0";
        let encoded = urlencoding::encode(model);
        let url = format!(
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/{}/invoke",
            encoded
        );
        assert!(url.contains("bedrock-runtime"));
        assert!(url.contains("/model/"));
        assert!(url.contains("/invoke"));
        assert!(url.contains("%3A"));
    }

    #[test]
    fn test_stream_url_format() {
        let model = "global.anthropic.claude-sonnet-4-5-20250929-v1:0";
        let encoded = urlencoding::encode(model);
        let url = format!(
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/{}/invoke-with-response-stream",
            encoded
        );
        assert!(url.contains("/invoke-with-response-stream"));
    }

    #[test]
    fn test_model_config() {
        let config = ModelConfig::bedrock();
        assert!(config.primary.contains("anthropic"));
        assert!(config.primary.contains("global"));
    }

    #[test]
    fn test_request_body() {
        let body = json!({
            "anthropic_version": ANTHROPIC_VERSION,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}],
        });
        assert_eq!(body["anthropic_version"], "bedrock-2023-05-31");
        assert_eq!(body["max_tokens"], 1024);
    }

    #[test]
    fn test_beta_header() {
        let beta_value = BetaFeature::Context1M.header_value();
        let mut body = json!({
            "anthropic_version": ANTHROPIC_VERSION,
            "max_tokens": 1024,
            "messages": [],
        });
        if let Some(obj) = body.as_object_mut() {
            obj.insert("anthropic_beta".to_string(), json!([beta_value]));
        }
        assert_eq!(body["anthropic_beta"][0], beta_value);
    }
}
