//! AWS Bedrock authentication strategy.

use std::fmt::Debug;

use super::env::{env_bool, env_opt, env_with_fallbacks_or};
use super::traits::AuthStrategy;

/// AWS Bedrock authentication strategy.
///
/// Supports multiple authentication methods:
/// - AWS credentials (access key + secret key + optional session token)
/// - Bearer token (AWS API key)
/// - LLM gateway passthrough (skip auth)
#[derive(Clone)]
pub struct BedrockStrategy {
    region: String,
    small_model_region: Option<String>,
    base_url: Option<String>,
    skip_auth: bool,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    bearer_token: Option<String>,
    inference_profile_arn: Option<String>,
    disable_caching: bool,
}

impl Debug for BedrockStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BedrockStrategy")
            .field("region", &self.region)
            .field("small_model_region", &self.small_model_region)
            .field("base_url", &self.base_url)
            .field("skip_auth", &self.skip_auth)
            .field("has_credentials", &self.access_key_id.is_some())
            .field("has_bearer_token", &self.bearer_token.is_some())
            .field("inference_profile_arn", &self.inference_profile_arn)
            .finish()
    }
}

impl BedrockStrategy {
    /// Create from environment variables.
    pub fn from_env() -> Option<Self> {
        if !env_bool("CLAUDE_CODE_USE_BEDROCK") {
            return None;
        }

        Some(Self {
            region: env_with_fallbacks_or(&["AWS_REGION", "AWS_DEFAULT_REGION"], "us-east-1"),
            small_model_region: env_opt("ANTHROPIC_SMALL_FAST_MODEL_AWS_REGION"),
            base_url: env_opt("ANTHROPIC_BEDROCK_BASE_URL"),
            skip_auth: env_bool("CLAUDE_CODE_SKIP_BEDROCK_AUTH"),
            access_key_id: env_opt("AWS_ACCESS_KEY_ID"),
            secret_access_key: env_opt("AWS_SECRET_ACCESS_KEY"),
            session_token: env_opt("AWS_SESSION_TOKEN"),
            bearer_token: env_opt("AWS_BEARER_TOKEN_BEDROCK"),
            inference_profile_arn: env_opt("AWS_BEDROCK_PROFILE_ARN"),
            disable_caching: env_bool("DISABLE_PROMPT_CACHING"),
        })
    }

    /// Create with explicit region.
    pub fn new(region: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            small_model_region: None,
            base_url: None,
            skip_auth: false,
            access_key_id: None,
            secret_access_key: None,
            session_token: None,
            bearer_token: None,
            inference_profile_arn: None,
            disable_caching: false,
        }
    }

    /// Set base URL for LLM gateway.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set separate region for small/fast model (cross-region inference).
    pub fn with_small_model_region(mut self, region: impl Into<String>) -> Self {
        self.small_model_region = Some(region.into());
        self
    }

    /// Skip AWS authentication (for gateways).
    pub fn skip_auth(mut self) -> Self {
        self.skip_auth = true;
        self
    }

    /// Set AWS credentials.
    pub fn with_credentials(
        mut self,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        self.access_key_id = Some(access_key_id.into());
        self.secret_access_key = Some(secret_access_key.into());
        self
    }

    /// Set session token.
    pub fn with_session_token(mut self, token: impl Into<String>) -> Self {
        self.session_token = Some(token.into());
        self
    }

    /// Set bearer token (alternative to AWS credentials).
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Set inference profile ARN for cross-region inference.
    pub fn with_inference_profile(mut self, arn: impl Into<String>) -> Self {
        self.inference_profile_arn = Some(arn.into());
        self
    }

    /// Disable prompt caching.
    pub fn disable_caching(mut self) -> Self {
        self.disable_caching = true;
        self
    }

    /// Get base URL for Bedrock API.
    pub fn get_base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| format!("https://bedrock-runtime.{}.amazonaws.com", self.region))
    }

    /// Get base URL for small model (may be different region).
    pub fn get_small_model_base_url(&self) -> String {
        let region = self.small_model_region.as_deref().unwrap_or(&self.region);
        format!("https://bedrock-runtime.{}.amazonaws.com", region)
    }

    /// Get the primary region.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Get the small model region.
    pub fn small_model_region(&self) -> &str {
        self.small_model_region.as_deref().unwrap_or(&self.region)
    }

    /// Check if prompt caching is disabled.
    pub fn is_caching_disabled(&self) -> bool {
        self.disable_caching
    }
}

impl AuthStrategy for BedrockStrategy {
    fn auth_header(&self) -> (&'static str, String) {
        if let Some(ref token) = self.bearer_token {
            return ("Authorization", format!("Bearer {}", token));
        }
        // For AWS SigV4, the actual signing happens at request time
        // This placeholder indicates Bedrock auth is needed
        ("x-bedrock-auth", "aws-sigv4".to_string())
    }

    fn extra_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        if !self.skip_auth
            && let Some(ref token) = self.session_token
        {
            headers.push(("x-amz-security-token".to_string(), token.clone()));
        }

        if let Some(ref arn) = self.inference_profile_arn {
            headers.push(("x-bedrock-inference-profile".to_string(), arn.clone()));
        }

        headers
    }

    fn name(&self) -> &'static str {
        "bedrock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bedrock_strategy_creation() {
        let strategy = BedrockStrategy::new("us-west-2");
        assert_eq!(strategy.region(), "us-west-2");
        assert_eq!(strategy.name(), "bedrock");
    }

    #[test]
    fn test_bedrock_base_url() {
        let strategy = BedrockStrategy::new("us-east-1");
        assert_eq!(
            strategy.get_base_url(),
            "https://bedrock-runtime.us-east-1.amazonaws.com"
        );
    }

    #[test]
    fn test_bedrock_cross_region() {
        let strategy = BedrockStrategy::new("us-east-1").with_small_model_region("us-west-2");

        assert_eq!(strategy.region(), "us-east-1");
        assert_eq!(strategy.small_model_region(), "us-west-2");
        assert!(strategy.get_small_model_base_url().contains("us-west-2"));
    }

    #[test]
    fn test_bedrock_bearer_token() {
        let strategy = BedrockStrategy::new("us-east-1").with_bearer_token("my-token");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "Authorization");
        assert!(value.contains("Bearer"));
    }

    #[test]
    fn test_bedrock_inference_profile() {
        let strategy = BedrockStrategy::new("us-east-1")
            .with_inference_profile("arn:aws:bedrock:us:123:inference-profile/xyz");

        let headers = strategy.extra_headers();
        assert!(
            headers
                .iter()
                .any(|(k, _)| k == "x-bedrock-inference-profile")
        );
    }
}
