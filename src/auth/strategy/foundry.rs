//! Microsoft Azure AI Foundry authentication strategy.

use std::fmt::Debug;

use super::env::{env_bool, env_opt, env_with_fallbacks};
use super::traits::AuthStrategy;

/// Microsoft Azure AI Foundry authentication strategy.
///
/// Uses Azure credentials for authentication with Claude models
/// deployed on Azure AI Foundry.
#[derive(Clone)]
pub struct FoundryStrategy {
    /// Azure resource name
    resource_name: String,
    /// Deployment name
    deployment_name: String,
    /// API version
    api_version: String,
    /// Base URL (auto-constructed if not provided)
    base_url: Option<String>,
    /// Skip Azure authentication (for LLM gateways)
    skip_auth: bool,
    /// Azure API key
    api_key: Option<String>,
    /// Azure AD token (alternative to API key)
    access_token: Option<String>,
}

impl Debug for FoundryStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoundryStrategy")
            .field("resource_name", &self.resource_name)
            .field("deployment_name", &self.deployment_name)
            .field("api_version", &self.api_version)
            .field("base_url", &self.base_url)
            .field("skip_auth", &self.skip_auth)
            .field("api_key", &self.api_key.as_ref().map(|_| "***"))
            .field("access_token", &self.access_token.as_ref().map(|_| "***"))
            .finish()
    }
}

impl FoundryStrategy {
    /// Default API version for Azure AI Foundry.
    pub const DEFAULT_API_VERSION: &'static str = "2024-06-01";

    /// Create a new Foundry strategy from environment variables.
    pub fn from_env() -> Option<Self> {
        if !env_bool("CLAUDE_CODE_USE_FOUNDRY") {
            return None;
        }

        let resource_name =
            env_with_fallbacks(&["AZURE_RESOURCE_NAME", "ANTHROPIC_FOUNDRY_RESOURCE"])?;

        let deployment_name =
            env_with_fallbacks(&["AZURE_DEPLOYMENT_NAME", "ANTHROPIC_FOUNDRY_DEPLOYMENT"])
                .unwrap_or_else(|| "claude-sonnet".to_string());

        let api_version =
            env_opt("AZURE_API_VERSION").unwrap_or_else(|| Self::DEFAULT_API_VERSION.to_string());

        Some(Self {
            resource_name,
            deployment_name,
            api_version,
            base_url: env_opt("ANTHROPIC_FOUNDRY_BASE_URL"),
            skip_auth: env_bool("CLAUDE_CODE_SKIP_FOUNDRY_AUTH"),
            api_key: env_opt("AZURE_API_KEY"),
            access_token: env_opt("AZURE_ACCESS_TOKEN"),
        })
    }

    /// Create with explicit configuration.
    pub fn new(resource_name: impl Into<String>, deployment_name: impl Into<String>) -> Self {
        Self {
            resource_name: resource_name.into(),
            deployment_name: deployment_name.into(),
            api_version: Self::DEFAULT_API_VERSION.to_string(),
            base_url: None,
            skip_auth: false,
            api_key: None,
            access_token: None,
        }
    }

    /// Set the API version.
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    /// Set the base URL (for LLM gateways).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Skip Azure authentication (for gateways that handle auth).
    pub fn skip_auth(mut self) -> Self {
        self.skip_auth = true;
        self
    }

    /// Set API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set access token (Azure AD).
    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Get the base URL for Azure AI Foundry API.
    pub fn get_base_url(&self) -> String {
        self.base_url.clone().unwrap_or_else(|| {
            format!(
                "https://{}.openai.azure.com/openai/deployments/{}",
                self.resource_name, self.deployment_name
            )
        })
    }

    /// Get the resource name.
    pub fn resource_name(&self) -> &str {
        &self.resource_name
    }

    /// Get the deployment name.
    pub fn deployment_name(&self) -> &str {
        &self.deployment_name
    }
}

impl AuthStrategy for FoundryStrategy {
    fn auth_header(&self) -> (&'static str, String) {
        if let Some(ref token) = self.access_token {
            ("Authorization", format!("Bearer {}", token))
        } else if let Some(ref key) = self.api_key {
            ("api-key", key.clone())
        } else {
            ("api-key", "<pending>".to_string())
        }
    }

    fn url_query_string(&self) -> Option<String> {
        Some(format!("api-version={}", self.api_version))
    }

    fn name(&self) -> &'static str {
        "foundry"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foundry_strategy_creation() {
        let strategy = FoundryStrategy::new("my-resource", "claude-sonnet");
        assert_eq!(strategy.resource_name(), "my-resource");
        assert_eq!(strategy.deployment_name(), "claude-sonnet");
        assert_eq!(strategy.name(), "foundry");
    }

    #[test]
    fn test_foundry_base_url() {
        let strategy = FoundryStrategy::new("my-resource", "claude-sonnet");
        let url = strategy.get_base_url();
        assert!(url.contains("my-resource"));
        assert!(url.contains("claude-sonnet"));

        let custom = FoundryStrategy::new("r", "d").with_base_url("https://my-gateway.com/foundry");
        assert_eq!(custom.get_base_url(), "https://my-gateway.com/foundry");
    }

    #[test]
    fn test_foundry_url_query() {
        let strategy = FoundryStrategy::new("r", "d");
        let query = strategy.url_query_string();
        assert!(query.is_some());
        assert!(query.unwrap().contains("api-version"));
    }

    #[test]
    fn test_foundry_auth_with_api_key() {
        let strategy = FoundryStrategy::new("r", "d").with_api_key("my-key");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "api-key");
        assert_eq!(value, "my-key");
    }

    #[test]
    fn test_foundry_auth_with_token() {
        let strategy = FoundryStrategy::new("r", "d").with_access_token("my-token");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "Authorization");
        assert!(value.contains("Bearer"));
    }
}
