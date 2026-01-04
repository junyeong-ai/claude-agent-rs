//! Anthropic Direct API adapter.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::config::ProviderConfig;
use super::traits::ProviderAdapter;
use crate::auth::{Credential, CredentialProvider, OAuthConfig};
use crate::client::messages::{
    CountTokensRequest, CountTokensResponse, CreateMessageRequest, ErrorResponse,
};
use crate::types::ApiResponse;
use crate::{Error, Result};

const BASE_URL: &str = "https://api.anthropic.com";

#[derive(Debug, Clone)]
enum AuthMethod {
    ApiKey(String),
    OAuth { token: String, config: OAuthConfig },
}

impl AuthMethod {
    fn from_credential(credential: Credential, oauth_config: Option<OAuthConfig>) -> Self {
        match credential {
            Credential::ApiKey(key) => Self::ApiKey(key),
            Credential::OAuth(oauth) => Self::OAuth {
                token: oauth.access_token,
                config: oauth_config.unwrap_or_default(),
            },
        }
    }

    fn update_token(&mut self, credential: Credential) {
        match credential {
            Credential::ApiKey(key) => *self = Self::ApiKey(key),
            Credential::OAuth(oauth) => {
                if let Self::OAuth { token, .. } = self {
                    *token = oauth.access_token;
                } else {
                    *self = Self::OAuth {
                        token: oauth.access_token,
                        config: OAuthConfig::default(),
                    };
                }
            }
        }
    }
}

pub struct AnthropicAdapter {
    config: ProviderConfig,
    base_url: String,
    auth: RwLock<AuthMethod>,
    credential_provider: Option<Arc<dyn CredentialProvider>>,
}

impl std::fmt::Debug for AnthropicAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicAdapter")
            .field("config", &self.config)
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl AnthropicAdapter {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            config,
            base_url: Self::base_url_from_env(),
            auth: RwLock::new(AuthMethod::ApiKey(Self::api_key_from_env())),
            credential_provider: None,
        }
    }

    fn api_key_from_env() -> String {
        std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
    }

    fn base_url_from_env() -> String {
        std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| BASE_URL.into())
    }

    pub fn from_credential(
        config: ProviderConfig,
        credential: Credential,
        oauth_config: Option<OAuthConfig>,
    ) -> Self {
        Self {
            config,
            base_url: Self::base_url_from_env(),
            auth: RwLock::new(AuthMethod::from_credential(credential, oauth_config)),
            credential_provider: None,
        }
    }

    pub fn from_credential_provider(
        config: ProviderConfig,
        credential: Credential,
        oauth_config: Option<OAuthConfig>,
        provider: Arc<dyn CredentialProvider>,
    ) -> Self {
        Self {
            config,
            base_url: Self::base_url_from_env(),
            auth: RwLock::new(AuthMethod::from_credential(credential, oauth_config)),
            credential_provider: Some(provider),
        }
    }

    pub fn with_api_key(self, key: impl Into<String>) -> Self {
        Self {
            auth: RwLock::new(AuthMethod::ApiKey(key.into())),
            ..self
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    fn build_endpoint_url(&self, auth: &AuthMethod, endpoint: &str) -> String {
        match auth {
            AuthMethod::OAuth { config, .. } => config.build_url(&self.base_url, endpoint),
            AuthMethod::ApiKey(_) => format!("{}{}", self.base_url, endpoint),
        }
    }

    fn build_headers(
        &self,
        req: reqwest::RequestBuilder,
        auth: &AuthMethod,
    ) -> reqwest::RequestBuilder {
        let mut r = match auth {
            AuthMethod::ApiKey(key) => req
                .header("x-api-key", key)
                .header("anthropic-version", &self.config.api_version)
                .header("content-type", "application/json"),
            AuthMethod::OAuth { token, config } => {
                config.apply_headers(req, token, &self.config.api_version, &self.config.beta)
            }
        };

        if let AuthMethod::ApiKey(_) = auth
            && let Some(beta) = self.config.beta.header_value()
        {
            r = r.header("anthropic-beta", beta);
        }

        for (k, v) in &self.config.extra_headers {
            r = r.header(k.as_str(), v.as_str());
        }

        r
    }

    /// Prepares the system prompt for OAuth users following Claude Code format.
    ///
    /// For OAuth authentication (CLI OAuth), the API requires system prompts as blocks:
    /// - First block MUST be the CLI identity prompt
    /// - Additional blocks can contain custom instructions (replace/append)
    ///
    /// Based on CLI binary analysis, the API validates that the first block
    /// is one of the allowed prompts.
    fn prepare_request_with_auth(
        &self,
        mut request: CreateMessageRequest,
        auth: &AuthMethod,
    ) -> CreateMessageRequest {
        if let AuthMethod::OAuth { .. } = auth {
            use crate::prompts::CLI_IDENTITY;

            // Build blocks array: CLI identity first, then any existing prompts
            let mut blocks = vec![crate::types::SystemBlock::uncached(CLI_IDENTITY)];

            match &request.system {
                Some(crate::types::SystemPrompt::Text(existing)) if !existing.is_empty() => {
                    // Don't duplicate if existing already starts with CLI identity
                    if !existing.starts_with(CLI_IDENTITY) {
                        blocks.push(crate::types::SystemBlock::uncached(existing));
                    }
                }
                Some(crate::types::SystemPrompt::Blocks(existing_blocks))
                    if !existing_blocks.is_empty() =>
                {
                    // Filter out any blocks that match the CLI identity to avoid duplication
                    for block in existing_blocks {
                        if block.text != CLI_IDENTITY {
                            blocks.push(block.clone());
                        }
                    }
                }
                _ => {}
            }

            request.system = Some(crate::types::SystemPrompt::Blocks(blocks));
        }
        request
    }

    async fn try_refresh_on_401(&self) -> Result<bool> {
        if let Some(ref provider) = self.credential_provider
            && provider.supports_refresh()
        {
            let new_credential = provider.refresh().await?;
            let mut auth = self.auth.write().await;
            auth.update_token(new_credential);
            return Ok(true);
        }
        Ok(false)
    }
}

#[async_trait]
impl ProviderAdapter for AnthropicAdapter {
    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn build_url(&self, _model: &str, _stream: bool) -> String {
        let auth = self.auth.read().await;
        self.build_endpoint_url(&auth, "/v1/messages")
    }

    async fn transform_request(&self, request: CreateMessageRequest) -> Result<serde_json::Value> {
        let auth = self.auth.read().await;
        let prepared = self.prepare_request_with_auth(request, &auth);
        serde_json::to_value(&prepared).map_err(|e| Error::InvalidRequest(e.to_string()))
    }

    fn transform_response(&self, response: serde_json::Value) -> Result<ApiResponse> {
        serde_json::from_value(response).map_err(|e| Error::Parse(e.to_string()))
    }

    async fn apply_auth_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let auth = self.auth.read().await;
        self.build_headers(req, &auth)
    }

    async fn send(
        &self,
        http: &reqwest::Client,
        request: CreateMessageRequest,
    ) -> Result<ApiResponse> {
        let (url, body) = {
            let auth = self.auth.read().await;
            let url = self.build_endpoint_url(&auth, "/v1/messages");
            let prepared = self.prepare_request_with_auth(request.clone(), &auth);
            (
                url,
                serde_json::to_value(&prepared)
                    .map_err(|e| Error::InvalidRequest(e.to_string()))?,
            )
        };

        let req = self.apply_auth_headers(http.post(&url)).await.json(&body);

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            if status == 401 && self.try_refresh_on_401().await? {
                let (url, body) = {
                    let auth = self.auth.read().await;
                    let url = self.build_endpoint_url(&auth, "/v1/messages");
                    let prepared = self.prepare_request_with_auth(request, &auth);
                    (
                        url,
                        serde_json::to_value(&prepared)
                            .map_err(|e| Error::InvalidRequest(e.to_string()))?,
                    )
                };
                let req = self.apply_auth_headers(http.post(&url)).await.json(&body);
                let response = req.send().await?;
                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let error: ErrorResponse = response.json().await?;
                    return Err(error.into_error(status));
                }
                let json: serde_json::Value = response.json().await?;
                return self.transform_response(json);
            }
            let error: ErrorResponse = response.json().await?;
            return Err(error.into_error(status));
        }

        let json: serde_json::Value = response.json().await?;
        self.transform_response(json)
    }

    async fn send_stream(
        &self,
        http: &reqwest::Client,
        mut request: CreateMessageRequest,
    ) -> Result<reqwest::Response> {
        request.stream = Some(true);

        let (url, body) = {
            let auth = self.auth.read().await;
            let url = self.build_endpoint_url(&auth, "/v1/messages");
            let prepared = self.prepare_request_with_auth(request.clone(), &auth);
            (
                url,
                serde_json::to_value(&prepared)
                    .map_err(|e| Error::InvalidRequest(e.to_string()))?,
            )
        };

        let req = self.apply_auth_headers(http.post(&url)).await.json(&body);

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            if status == 401 && self.try_refresh_on_401().await? {
                let (url, body) = {
                    let auth = self.auth.read().await;
                    let url = self.build_endpoint_url(&auth, "/v1/messages");
                    let prepared = self.prepare_request_with_auth(request, &auth);
                    (
                        url,
                        serde_json::to_value(&prepared)
                            .map_err(|e| Error::InvalidRequest(e.to_string()))?,
                    )
                };
                let req = self.apply_auth_headers(http.post(&url)).await.json(&body);
                let response = req.send().await?;
                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let error: ErrorResponse = response.json().await?;
                    return Err(error.into_error(status));
                }
                return Ok(response);
            }
            let error: ErrorResponse = response.json().await?;
            return Err(error.into_error(status));
        }

        Ok(response)
    }

    async fn refresh_credentials(&self) -> Result<()> {
        self.try_refresh_on_401().await?;
        Ok(())
    }

    async fn count_tokens(
        &self,
        http: &reqwest::Client,
        request: CountTokensRequest,
    ) -> Result<CountTokensResponse> {
        const ENDPOINT: &str = "/v1/messages/count_tokens";
        let (url, body) = {
            let auth = self.auth.read().await;
            let url = self.build_endpoint_url(&auth, ENDPOINT);
            (url, serde_json::to_value(&request)?)
        };

        let req = self.apply_auth_headers(http.post(&url)).await.json(&body);

        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            if status == 401 && self.try_refresh_on_401().await? {
                let req = {
                    let auth = self.auth.read().await;
                    let url = self.build_endpoint_url(&auth, ENDPOINT);
                    self.build_headers(http.post(&url), &auth).json(&body)
                };
                let response = req.send().await?;
                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let error: ErrorResponse = response.json().await?;
                    return Err(error.into_error(status));
                }
                return Ok(response.json().await?);
            }
            let error: ErrorResponse = response.json().await?;
            return Err(error.into_error(status));
        }

        Ok(response.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::adapter::{BetaConfig, BetaFeature, ModelConfig};
    use crate::types::Message;

    #[tokio::test]
    async fn test_build_url() {
        let adapter = AnthropicAdapter::new(ProviderConfig::new(ModelConfig::anthropic()));
        let url = adapter.build_url("claude-sonnet-4-5", false).await;
        assert!(url.contains("/v1/messages"));
    }

    #[tokio::test]
    async fn test_transform_request() {
        let adapter = AnthropicAdapter::new(ProviderConfig::new(ModelConfig::anthropic()));
        let request = CreateMessageRequest::new("claude-sonnet-4-5", vec![Message::user("Hello")]);
        let body = adapter.transform_request(request).await.unwrap();
        assert!(body.get("model").is_some());
        assert!(body.get("messages").is_some());
    }

    #[tokio::test]
    async fn test_oauth_url_params() {
        let credential = Credential::oauth("test-token");
        let adapter = AnthropicAdapter::from_credential(
            ProviderConfig::new(ModelConfig::anthropic()),
            credential,
            None,
        );
        let url = adapter.build_url("model", false).await;
        assert!(url.contains("beta=true"));
    }

    #[tokio::test]
    async fn test_oauth_system_prompt() {
        let credential = Credential::oauth("test-token");
        let adapter = AnthropicAdapter::from_credential(
            ProviderConfig::new(ModelConfig::anthropic()),
            credential,
            None,
        );
        let request = CreateMessageRequest::new("model", vec![Message::user("Hi")]);
        let body = adapter.transform_request(request).await.unwrap();
        let system_blocks = body
            .get("system")
            .and_then(|v| v.as_array())
            .expect("OAuth should produce system blocks");
        let first_text = system_blocks[0]
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(first_text.contains("Claude Code"));
    }

    #[test]
    fn test_api_key_with_beta() {
        let config = ProviderConfig::new(ModelConfig::anthropic())
            .with_beta(BetaFeature::InterleavedThinking)
            .with_beta(BetaFeature::ContextManagement);

        let adapter = AnthropicAdapter::new(config);
        assert!(adapter.config.beta.has(BetaFeature::InterleavedThinking));
        assert!(adapter.config.beta.has(BetaFeature::ContextManagement));
    }

    #[test]
    fn test_api_key_with_custom_beta() {
        let beta = BetaConfig::new().with_custom("new-feature-2026-01-01");
        let config = ProviderConfig::new(ModelConfig::anthropic()).with_beta_config(beta);

        let adapter = AnthropicAdapter::new(config);
        let header = adapter.config.beta.header_value().unwrap();
        assert!(header.contains("new-feature-2026-01-01"));
    }

    #[tokio::test]
    async fn test_oauth_prepends_cli_identity_to_system_prompt() {
        let credential = Credential::oauth("test-token");
        let adapter = AnthropicAdapter::from_credential(
            ProviderConfig::new(ModelConfig::anthropic()),
            credential,
            None,
        );

        let request = CreateMessageRequest::new("model", vec![Message::user("Hi")])
            .with_system("Custom user system prompt");

        let body = adapter.transform_request(request).await.unwrap();
        let system_blocks = body
            .get("system")
            .and_then(|v| v.as_array())
            .expect("OAuth should produce system blocks");

        assert!(system_blocks.len() >= 2, "Should have at least 2 blocks");

        let first_text = system_blocks[0]
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            first_text.starts_with("You are Claude Code"),
            "First block should be Claude Code identity: {}",
            first_text
        );

        let second_text = system_blocks[1]
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(
            second_text, "Custom user system prompt",
            "Second block should preserve original"
        );
    }

    #[tokio::test]
    async fn test_api_key_does_not_modify_system_prompt() {
        let adapter = AnthropicAdapter::new(ProviderConfig::new(ModelConfig::anthropic()))
            .with_api_key("sk-test");

        // Create request with existing system prompt
        let request = CreateMessageRequest::new("model", vec![Message::user("Hi")])
            .with_system("Custom user system prompt");

        let body = adapter.transform_request(request).await.unwrap();
        let system = body.get("system").and_then(|v| v.as_str()).unwrap_or("");

        // For API key auth, system prompt should be unchanged
        assert_eq!(
            system, "Custom user system prompt",
            "API key auth should not modify system prompt"
        );
        // Should NOT contain CLI identity
        assert!(
            !system.contains("Claude Code"),
            "API key auth should not add CLI identity: {}",
            system
        );
    }
}
