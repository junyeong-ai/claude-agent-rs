//! Client configuration.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::Result;
use crate::auth::{
    ApiKeyStrategy, AuthStrategy, BedrockStrategy, ChainProvider, ClaudeCliProvider, Credential,
    CredentialProvider, ExplicitProvider, FoundryStrategy, OAuthConfig, OAuthStrategy,
    VertexStrategy,
};
use crate::mcp::McpServerConfig;

use super::gateway::GatewayConfig;
use super::models::{DEFAULT_MODEL, DEFAULT_SMALL_MODEL, ModelConfig};
use super::network::NetworkConfig;

/// Default Anthropic API base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const DEFAULT_MAX_TOKENS: u32 = 8192;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);
pub const DEFAULT_API_VERSION: &str = "2023-06-01";

/// Client configuration.
#[derive(Clone)]
pub struct Config {
    /// Authentication strategy.
    pub auth_strategy: Arc<dyn AuthStrategy>,
    /// Base URL for API requests.
    pub base_url: String,
    /// Default model to use.
    pub model: String,
    /// Small/fast model for lightweight tasks.
    pub small_model: String,
    /// Default max tokens.
    pub max_tokens: u32,
    /// Request timeout.
    pub timeout: Duration,
    /// API version header.
    pub api_version: String,
    /// Gateway configuration (custom headers, auth override).
    pub gateway: Option<GatewayConfig>,
}

impl Config {
    /// Create configuration from environment.
    pub async fn from_env() -> Result<Self> {
        let provider = ChainProvider::default();
        let credential = provider.resolve().await?;
        let auth_strategy = credential_to_strategy(credential, None);
        let gateway = GatewayConfig::from_env();

        Ok(Self {
            auth_strategy,
            base_url: gateway
                .as_ref()
                .and_then(|g| g.base_url.clone())
                .unwrap_or_else(|| {
                    std::env::var("ANTHROPIC_BASE_URL")
                        .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
                }),
            model: std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            small_model: std::env::var("ANTHROPIC_SMALL_FAST_MODEL")
                .unwrap_or_else(|_| DEFAULT_SMALL_MODEL.to_string()),
            max_tokens: DEFAULT_MAX_TOKENS,
            timeout: DEFAULT_TIMEOUT,
            api_version: DEFAULT_API_VERSION.to_string(),
            gateway,
        })
    }
}

/// Convert a credential to an authentication strategy.
pub(crate) fn credential_to_strategy(
    credential: Credential,
    oauth_config: Option<OAuthConfig>,
) -> Arc<dyn AuthStrategy> {
    match credential {
        Credential::ApiKey(key) => Arc::new(ApiKeyStrategy::new(key)),
        Credential::OAuth(oauth_cred) => {
            let config = oauth_config.unwrap_or_else(OAuthConfig::from_env);
            Arc::new(OAuthStrategy::with_config(oauth_cred, config))
        }
    }
}

/// Cloud provider type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloudProvider {
    /// Direct Anthropic API
    #[default]
    Anthropic,
    /// AWS Bedrock
    Bedrock,
    /// Google Vertex AI
    Vertex,
    /// Microsoft Azure AI Foundry
    Foundry,
}

/// Builder for client configuration.
#[derive(Default)]
pub struct ClientBuilder {
    credential_provider: Option<Box<dyn CredentialProvider>>,
    oauth_config: Option<OAuthConfig>,
    cloud_provider: Option<CloudProvider>,
    bedrock_strategy: Option<BedrockStrategy>,
    vertex_strategy: Option<VertexStrategy>,
    foundry_strategy: Option<FoundryStrategy>,
    gateway_config: Option<GatewayConfig>,
    network_config: Option<NetworkConfig>,
    base_url: Option<String>,
    model: Option<String>,
    small_model: Option<String>,
    max_tokens: Option<u32>,
    timeout: Option<Duration>,
    mcp_servers: HashMap<String, McpServerConfig>,
}

impl ClientBuilder {
    /// Set API key for authentication.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.credential_provider = Some(Box::new(ExplicitProvider::api_key(key)));
        self
    }

    /// Set OAuth token for authentication.
    pub fn oauth_token(mut self, token: impl Into<String>) -> Self {
        self.credential_provider = Some(Box::new(ExplicitProvider::oauth(token)));
        self
    }

    /// Use Claude Code CLI credentials.
    pub fn from_claude_cli(mut self) -> Self {
        self.credential_provider = Some(Box::new(ClaudeCliProvider::new()));
        self
    }

    /// Auto-resolve credentials (environment → CLI → Bedrock → Vertex → Foundry).
    pub fn auto_resolve(mut self) -> Self {
        if let Some(bedrock) = BedrockStrategy::from_env() {
            self.cloud_provider = Some(CloudProvider::Bedrock);
            self.bedrock_strategy = Some(bedrock);
            return self;
        }

        if let Some(vertex) = VertexStrategy::from_env() {
            self.cloud_provider = Some(CloudProvider::Vertex);
            self.vertex_strategy = Some(vertex);
            return self;
        }

        if let Some(foundry) = FoundryStrategy::from_env() {
            self.cloud_provider = Some(CloudProvider::Foundry);
            self.foundry_strategy = Some(foundry);
            return self;
        }

        self.credential_provider = Some(Box::new(ChainProvider::default()));
        self
    }

    /// Use AWS Bedrock.
    pub fn from_bedrock(mut self) -> Self {
        if let Some(strategy) = BedrockStrategy::from_env() {
            self.cloud_provider = Some(CloudProvider::Bedrock);
            self.bedrock_strategy = Some(strategy);
        }
        self
    }

    /// Use AWS Bedrock with explicit configuration.
    pub fn bedrock(mut self, region: impl Into<String>) -> Self {
        self.cloud_provider = Some(CloudProvider::Bedrock);
        self.bedrock_strategy = Some(BedrockStrategy::new(region));
        self
    }

    /// Use AWS Bedrock with custom strategy.
    pub fn bedrock_strategy(mut self, strategy: BedrockStrategy) -> Self {
        self.cloud_provider = Some(CloudProvider::Bedrock);
        self.bedrock_strategy = Some(strategy);
        self
    }

    /// Use Google Vertex AI.
    pub fn from_vertex(mut self) -> Self {
        if let Some(strategy) = VertexStrategy::from_env() {
            self.cloud_provider = Some(CloudProvider::Vertex);
            self.vertex_strategy = Some(strategy);
        }
        self
    }

    /// Use Google Vertex AI with explicit configuration.
    pub fn vertex(mut self, project_id: impl Into<String>, region: impl Into<String>) -> Self {
        self.cloud_provider = Some(CloudProvider::Vertex);
        self.vertex_strategy = Some(VertexStrategy::new(project_id, region));
        self
    }

    /// Use Google Vertex AI with custom strategy.
    pub fn vertex_strategy(mut self, strategy: VertexStrategy) -> Self {
        self.cloud_provider = Some(CloudProvider::Vertex);
        self.vertex_strategy = Some(strategy);
        self
    }

    /// Use Microsoft Azure AI Foundry.
    pub fn from_foundry(mut self) -> Self {
        if let Some(strategy) = FoundryStrategy::from_env() {
            self.cloud_provider = Some(CloudProvider::Foundry);
            self.foundry_strategy = Some(strategy);
        }
        self
    }

    /// Use Microsoft Azure AI Foundry with explicit configuration.
    pub fn foundry(mut self, resource_name: impl Into<String>) -> Self {
        self.cloud_provider = Some(CloudProvider::Foundry);
        self.foundry_strategy = Some(FoundryStrategy::new(resource_name));
        self
    }

    /// Use Microsoft Azure AI Foundry with resource and deployment.
    pub fn foundry_with_deployment(
        mut self,
        resource_name: impl Into<String>,
        deployment_name: impl Into<String>,
    ) -> Self {
        self.cloud_provider = Some(CloudProvider::Foundry);
        self.foundry_strategy =
            Some(FoundryStrategy::new(resource_name).with_deployment(deployment_name));
        self
    }

    /// Use Microsoft Azure AI Foundry with custom strategy.
    pub fn foundry_strategy(mut self, strategy: FoundryStrategy) -> Self {
        self.cloud_provider = Some(CloudProvider::Foundry);
        self.foundry_strategy = Some(strategy);
        self
    }

    /// Use custom credential provider.
    pub fn credential_provider<P: CredentialProvider + 'static>(mut self, provider: P) -> Self {
        self.credential_provider = Some(Box::new(provider));
        self
    }

    /// Set custom OAuth configuration.
    pub fn oauth_config(mut self, config: OAuthConfig) -> Self {
        self.oauth_config = Some(config);
        self
    }

    /// Add a beta flag (OAuth only).
    pub fn add_beta_flag(mut self, flag: impl Into<String>) -> Self {
        let config = self.oauth_config.get_or_insert_with(OAuthConfig::from_env);
        config.beta_flags.push(flag.into());
        self
    }

    /// Set user agent (OAuth only).
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        let config = self.oauth_config.get_or_insert_with(OAuthConfig::from_env);
        config.user_agent = ua.into();
        self
    }

    /// Set Claude Code system prompt (OAuth only).
    pub fn claude_code_prompt(mut self, prompt: impl Into<String>) -> Self {
        let config = self.oauth_config.get_or_insert_with(OAuthConfig::from_env);
        config.system_prompt = prompt.into();
        self
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the default model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the small/fast model for lightweight tasks.
    pub fn small_model(mut self, model: impl Into<String>) -> Self {
        self.small_model = Some(model.into());
        self
    }

    /// Set the default max tokens.
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Add an MCP server.
    pub fn mcp_server(mut self, name: impl Into<String>, config: McpServerConfig) -> Self {
        self.mcp_servers.insert(name.into(), config);
        self
    }

    /// Configure LLM gateway (custom endpoint, headers, auth).
    pub fn gateway(mut self, config: GatewayConfig) -> Self {
        // Gateway base_url overrides provider base_url
        if let Some(ref url) = config.base_url {
            self.base_url = Some(url.clone());
        }
        self.gateway_config = Some(config);
        self
    }

    /// Configure network settings (proxy, TLS, certificates).
    pub fn network(mut self, config: NetworkConfig) -> Self {
        self.network_config = Some(config);
        self
    }

    /// Set proxy for network requests.
    pub fn proxy(mut self, https_url: impl Into<String>) -> Self {
        let proxy = super::network::ProxyConfig::https(https_url);
        let network = self
            .network_config
            .get_or_insert_with(NetworkConfig::default);
        network.proxy = Some(proxy);
        self
    }

    /// Get MCP server configurations.
    pub fn mcp_servers(&self) -> &HashMap<String, McpServerConfig> {
        &self.mcp_servers
    }

    /// Take MCP server configurations.
    pub fn take_mcp_servers(&mut self) -> HashMap<String, McpServerConfig> {
        std::mem::take(&mut self.mcp_servers)
    }

    /// Resolve cloud provider strategy.
    fn resolve_cloud_strategy(&mut self) -> Option<(Arc<dyn AuthStrategy>, String)> {
        match self.cloud_provider {
            Some(CloudProvider::Bedrock) => {
                let strategy = self.bedrock_strategy.take().unwrap_or_else(|| {
                    BedrockStrategy::from_env().unwrap_or_else(|| BedrockStrategy::new("us-east-1"))
                });
                Some((Arc::new(strategy.clone()), strategy.get_base_url()))
            }
            Some(CloudProvider::Vertex) => {
                let strategy = self.vertex_strategy.take().unwrap_or_else(|| {
                    VertexStrategy::from_env()
                        .unwrap_or_else(|| VertexStrategy::new("default", "us-central1"))
                });
                Some((Arc::new(strategy.clone()), strategy.get_base_url()))
            }
            Some(CloudProvider::Foundry) => {
                let strategy = self.foundry_strategy.take().unwrap_or_else(|| {
                    FoundryStrategy::from_env().unwrap_or_else(|| FoundryStrategy::new("default"))
                });
                Some((Arc::new(strategy.clone()), strategy.get_base_url()))
            }
            _ => None,
        }
    }

    /// Resolve model configuration from builder or environment.
    fn resolve_models(&mut self) -> (String, String) {
        // Get provider-specific defaults
        let defaults = match self.cloud_provider {
            Some(CloudProvider::Bedrock) => ModelConfig::for_bedrock(),
            Some(CloudProvider::Vertex) => ModelConfig::for_vertex(),
            _ => ModelConfig::from_env(),
        };

        let model = self.model.take().unwrap_or(defaults.primary);
        let small_model = self.small_model.take().unwrap_or(defaults.small);
        (model, small_model)
    }

    /// Build Config from resolved auth strategy and base URL.
    fn build_config(
        &mut self,
        auth_strategy: Arc<dyn AuthStrategy>,
        default_base_url: String,
    ) -> Config {
        let (model, small_model) = self.resolve_models();
        Config {
            auth_strategy,
            base_url: self.base_url.take().unwrap_or(default_base_url),
            model,
            small_model,
            max_tokens: self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            timeout: self.timeout.unwrap_or(DEFAULT_TIMEOUT),
            api_version: DEFAULT_API_VERSION.to_string(),
            gateway: self.gateway_config.take(),
        }
    }

    /// Build the client (blocking credential resolution).
    pub fn build(mut self) -> Result<super::Client> {
        let (auth_strategy, default_base_url) =
            if let Some(resolved) = self.resolve_cloud_strategy() {
                resolved
            } else {
                let provider = self
                    .credential_provider
                    .take()
                    .unwrap_or_else(|| Box::new(ChainProvider::default()));
                let credential = futures::executor::block_on(provider.resolve())?;
                let strategy = credential_to_strategy(credential, self.oauth_config.take());
                (strategy, DEFAULT_BASE_URL.to_string())
            };

        let config = self.build_config(auth_strategy, default_base_url);

        if let Some(network) = self.network_config.take() {
            super::Client::with_network(config, &network)
        } else {
            super::Client::new(config)
        }
    }

    /// Build the client asynchronously.
    pub async fn build_async(mut self) -> Result<super::Client> {
        let (auth_strategy, default_base_url) =
            if let Some(resolved) = self.resolve_cloud_strategy() {
                resolved
            } else {
                let provider = self
                    .credential_provider
                    .take()
                    .unwrap_or_else(|| Box::new(ChainProvider::default()));
                let credential = provider.resolve().await?;
                let strategy = credential_to_strategy(credential, self.oauth_config.take());
                (strategy, DEFAULT_BASE_URL.to_string())
            };

        let config = self.build_config(auth_strategy, default_base_url);

        if let Some(network) = self.network_config.take() {
            super::Client::with_network(config, &network)
        } else {
            super::Client::new(config)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_api_key() {
        let builder = ClientBuilder::default().api_key("test-key");
        assert!(builder.credential_provider.is_some());
    }

    #[test]
    fn test_builder_from_claude_cli() {
        let builder = ClientBuilder::default().from_claude_cli();
        assert!(builder.credential_provider.is_some());
    }

    #[test]
    fn test_builder_oauth_config() {
        let builder = ClientBuilder::default()
            .add_beta_flag("new-flag")
            .user_agent("my-app/1.0");

        assert!(builder.oauth_config.is_some());
        let config = builder.oauth_config.unwrap();
        assert!(config.beta_flags.contains(&"new-flag".to_string()));
        assert_eq!(config.user_agent, "my-app/1.0");
    }
}
