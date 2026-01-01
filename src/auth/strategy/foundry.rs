//! Microsoft Azure AI Foundry authentication strategy.

use std::fmt::Debug;

use super::env::{env_bool, env_opt, env_with_fallbacks};
use super::traits::AuthStrategy;

/// Microsoft Azure AI Foundry authentication strategy.
///
/// Supports:
/// - API key authentication
/// - Microsoft Entra ID (Azure AD) token authentication
/// - LLM gateway passthrough
#[derive(Clone)]
pub struct FoundryStrategy {
    resource_name: String,
    deployment_name: Option<String>,
    api_version: String,
    base_url: Option<String>,
    skip_auth: bool,
    api_key: Option<String>,
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
            .field("has_api_key", &self.api_key.is_some())
            .field("has_access_token", &self.access_token.is_some())
            .finish()
    }
}

impl FoundryStrategy {
    /// Default API version for Azure AI Foundry.
    pub const DEFAULT_API_VERSION: &'static str = "2024-06-01";

    /// Create from environment variables.
    pub fn from_env() -> Option<Self> {
        if !env_bool("CLAUDE_CODE_USE_FOUNDRY") {
            return None;
        }

        let resource_name =
            env_with_fallbacks(&["ANTHROPIC_FOUNDRY_RESOURCE", "AZURE_RESOURCE_NAME"])?;

        let deployment_name = env_with_fallbacks(&[
            "ANTHROPIC_FOUNDRY_DEPLOYMENT",
            "AZURE_DEPLOYMENT_NAME",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
        ]);

        let api_version =
            env_opt("AZURE_API_VERSION").unwrap_or_else(|| Self::DEFAULT_API_VERSION.to_string());

        Some(Self {
            resource_name,
            deployment_name,
            api_version,
            base_url: env_opt("ANTHROPIC_FOUNDRY_BASE_URL"),
            skip_auth: env_bool("CLAUDE_CODE_SKIP_FOUNDRY_AUTH"),
            api_key: env_with_fallbacks(&["ANTHROPIC_FOUNDRY_API_KEY", "AZURE_API_KEY"]),
            access_token: env_opt("AZURE_ACCESS_TOKEN"),
        })
    }

    /// Create with explicit resource name.
    pub fn new(resource_name: impl Into<String>) -> Self {
        Self {
            resource_name: resource_name.into(),
            deployment_name: None,
            api_version: Self::DEFAULT_API_VERSION.to_string(),
            base_url: None,
            skip_auth: false,
            api_key: None,
            access_token: None,
        }
    }

    /// Set the deployment name.
    pub fn with_deployment(mut self, name: impl Into<String>) -> Self {
        self.deployment_name = Some(name.into());
        self
    }

    /// Set the API version.
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    /// Set base URL for LLM gateway.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Skip Azure authentication (for gateways).
    pub fn skip_auth(mut self) -> Self {
        self.skip_auth = true;
        self
    }

    /// Set API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set access token (Azure AD / Microsoft Entra ID).
    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Get base URL for Azure AI Foundry API.
    /// Uses services.ai.azure.com endpoint format.
    pub fn get_base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| format!("https://{}.services.ai.azure.com", self.resource_name))
    }

    /// Get full URL for a specific model/deployment.
    pub fn get_model_url(&self, model: &str) -> String {
        let base = self.get_base_url();
        let deployment = self.deployment_name.as_deref().unwrap_or(model);
        format!("{}/models/{}", base, deployment)
    }

    /// Get the resource name.
    pub fn resource_name(&self) -> &str {
        &self.resource_name
    }

    /// Get the deployment name.
    pub fn deployment_name(&self) -> Option<&str> {
        self.deployment_name.as_deref()
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
        let strategy = FoundryStrategy::new("my-resource");
        assert_eq!(strategy.resource_name(), "my-resource");
        assert_eq!(strategy.name(), "foundry");
    }

    #[test]
    fn test_foundry_base_url() {
        let strategy = FoundryStrategy::new("my-resource");
        let url = strategy.get_base_url();
        // Must use services.ai.azure.com format
        assert!(url.contains("services.ai.azure.com"));
        assert!(url.contains("my-resource"));
    }

    #[test]
    fn test_foundry_custom_base_url() {
        let strategy = FoundryStrategy::new("r").with_base_url("https://my-gateway.com");
        assert_eq!(strategy.get_base_url(), "https://my-gateway.com");
    }

    #[test]
    fn test_foundry_url_query() {
        let strategy = FoundryStrategy::new("r");
        let query = strategy.url_query_string();
        assert!(query.is_some());
        assert!(query.unwrap().contains("api-version"));
    }

    #[test]
    fn test_foundry_auth_with_api_key() {
        let strategy = FoundryStrategy::new("r").with_api_key("my-key");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "api-key");
        assert_eq!(value, "my-key");
    }

    #[test]
    fn test_foundry_auth_with_token() {
        let strategy = FoundryStrategy::new("r").with_access_token("my-token");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "Authorization");
        assert!(value.contains("Bearer"));
    }

    #[test]
    fn test_foundry_model_url() {
        let strategy = FoundryStrategy::new("my-resource").with_deployment("claude-sonnet");
        let url = strategy.get_model_url("claude-sonnet-4-5");
        assert!(url.contains("models/claude-sonnet"));
    }
}
