//! AWS Bedrock authentication strategy.

use std::fmt::Debug;

use crate::client::messages::CreateMessageRequest;
use crate::types::SystemPrompt;

use super::env::{env_bool, env_opt, env_with_fallbacks_or};
use super::traits::AuthStrategy;

/// AWS Bedrock authentication strategy.
///
/// Uses AWS credentials (access key, secret key, session token) for authentication.
/// Supports both direct AWS credentials and LLM gateway proxies.
#[derive(Clone)]
pub struct BedrockStrategy {
    /// AWS region
    region: String,
    /// Base URL (auto-constructed if not provided)
    base_url: Option<String>,
    /// Skip AWS authentication (for LLM gateways)
    skip_auth: bool,
    /// AWS access key ID
    access_key_id: Option<String>,
    /// AWS secret access key
    secret_access_key: Option<String>,
    /// AWS session token (optional)
    session_token: Option<String>,
}

impl Debug for BedrockStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BedrockStrategy")
            .field("region", &self.region)
            .field("base_url", &self.base_url)
            .field("skip_auth", &self.skip_auth)
            .field("access_key_id", &self.access_key_id.as_ref().map(|_| "***"))
            .finish()
    }
}

impl BedrockStrategy {
    /// Create a new Bedrock strategy from environment variables.
    pub fn from_env() -> Option<Self> {
        if !env_bool("CLAUDE_CODE_USE_BEDROCK") {
            return None;
        }

        Some(Self {
            region: env_with_fallbacks_or(&["AWS_REGION", "AWS_DEFAULT_REGION"], "us-east-1"),
            base_url: env_opt("ANTHROPIC_BEDROCK_BASE_URL"),
            skip_auth: env_bool("CLAUDE_CODE_SKIP_BEDROCK_AUTH"),
            access_key_id: env_opt("AWS_ACCESS_KEY_ID"),
            secret_access_key: env_opt("AWS_SECRET_ACCESS_KEY"),
            session_token: env_opt("AWS_SESSION_TOKEN"),
        })
    }

    /// Create with explicit configuration.
    pub fn new(region: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            base_url: None,
            skip_auth: false,
            access_key_id: None,
            secret_access_key: None,
            session_token: None,
        }
    }

    /// Set the base URL (for LLM gateways).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Skip AWS authentication (for gateways that handle auth).
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

    /// Get the base URL for Bedrock API.
    pub fn get_base_url(&self) -> String {
        self.base_url.clone().unwrap_or_else(|| {
            format!("https://bedrock-runtime.{}.amazonaws.com", self.region)
        })
    }

    /// Get the region.
    pub fn region(&self) -> &str {
        &self.region
    }
}

impl AuthStrategy for BedrockStrategy {
    fn auth_header(&self) -> (&'static str, String) {
        // Bedrock uses AWS Signature V4, not a simple header
        // For now, return empty - actual signing happens in request building
        ("x-bedrock-auth", "aws-sigv4".to_string())
    }

    fn extra_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        // Add AWS-specific headers if not skipping auth
        if !self.skip_auth {
            if let Some(ref token) = self.session_token {
                headers.push(("x-amz-security-token".to_string(), token.clone()));
            }
        }

        headers
    }

    fn url_query_string(&self) -> Option<String> {
        None
    }

    fn prepare_system_prompt(
        &self,
        existing: Option<SystemPrompt>,
    ) -> Option<SystemPrompt> {
        // Bedrock doesn't require special system prompt handling
        existing
    }

    fn prepare_metadata(&self) -> Option<crate::client::messages::RequestMetadata> {
        None
    }

    fn prepare_request(&self, request: CreateMessageRequest) -> CreateMessageRequest {
        request
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

        let custom = BedrockStrategy::new("us-east-1")
            .with_base_url("https://my-gateway.com/bedrock");
        assert_eq!(custom.get_base_url(), "https://my-gateway.com/bedrock");
    }

    #[test]
    fn test_bedrock_skip_auth() {
        let strategy = BedrockStrategy::new("us-east-1").skip_auth();
        assert!(strategy.skip_auth);
    }
}
