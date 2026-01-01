//! Anthropic API client.

mod config;
mod error;
pub mod gateway;
pub mod messages;
pub mod models;
pub mod network;
mod streaming;

pub use config::{ClientBuilder, CloudProvider, Config, DEFAULT_BASE_URL};
pub use error::ClientError;
pub use gateway::GatewayConfig;
pub use messages::MessagesClient;
pub use models::ModelConfig;
pub use network::{ClientCertConfig, NetworkConfig, ProxyConfig};
pub use streaming::{StreamItem, StreamParser};

use crate::auth::{ChainProvider, CredentialProvider};
use crate::{Error, Result};

/// Main API client.
#[derive(Clone)]
pub struct Client {
    config: Config,
    http: reqwest::Client,
}

impl Client {
    /// Create a new client with configuration.
    pub fn new(config: Config) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(Error::Network)?;

        Ok(Self { config, http })
    }

    /// Create a new client with network configuration.
    pub fn with_network(config: Config, network: &NetworkConfig) -> Result<Self> {
        let builder = reqwest::Client::builder().timeout(config.timeout);

        let builder = network
            .apply_to_builder(builder)
            .map_err(|e| Error::Config(e.to_string()))?;

        let http = builder.build().map_err(Error::Network)?;

        Ok(Self { config, http })
    }

    /// Create from environment (tries ANTHROPIC_API_KEY then CLI).
    pub fn from_env() -> Result<Self> {
        let provider = ChainProvider::default();
        let credential = futures::executor::block_on(provider.resolve())?;
        let auth_strategy = config::credential_to_strategy(credential, None);
        let gateway = GatewayConfig::from_env();

        let config = Config {
            auth_strategy,
            base_url: gateway
                .as_ref()
                .and_then(|g| g.base_url.clone())
                .unwrap_or_else(|| {
                    std::env::var("ANTHROPIC_BASE_URL")
                        .unwrap_or_else(|_| config::DEFAULT_BASE_URL.to_string())
                }),
            model: std::env::var("ANTHROPIC_MODEL")
                .unwrap_or_else(|_| models::DEFAULT_MODEL.to_string()),
            small_model: std::env::var("ANTHROPIC_SMALL_FAST_MODEL")
                .unwrap_or_else(|_| models::DEFAULT_SMALL_MODEL.to_string()),
            max_tokens: config::DEFAULT_MAX_TOKENS,
            timeout: config::DEFAULT_TIMEOUT,
            api_version: config::DEFAULT_API_VERSION.to_string(),
            gateway,
        };

        let network = NetworkConfig::from_env();
        if network.is_configured() {
            Self::with_network(config, &network)
        } else {
            Self::new(config)
        }
    }

    /// Create from environment asynchronously.
    pub async fn from_env_async() -> Result<Self> {
        let config = Config::from_env().await?;
        let network = NetworkConfig::from_env();

        if network.is_configured() {
            Self::with_network(config, &network)
        } else {
            Self::new(config)
        }
    }

    /// Create a new client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Simple query for one-shot requests.
    pub async fn query(&self, prompt: &str) -> Result<String> {
        let request = messages::CreateMessageRequest::new(
            &self.config.model,
            vec![crate::types::Message::user(prompt)],
        )
        .with_max_tokens(self.config.max_tokens);

        let response = MessagesClient::new(self).create(request).await?;
        Ok(response.text())
    }

    /// Streaming query for one-shot requests.
    pub async fn stream(
        &self,
        prompt: &str,
    ) -> Result<impl futures::Stream<Item = Result<String>> + Send + 'static + use<>> {
        let request = messages::CreateMessageRequest::new(
            &self.config.model,
            vec![crate::types::Message::user(prompt)],
        )
        .with_max_tokens(self.config.max_tokens);

        let stream = MessagesClient::new(self).create_stream(request).await?;

        Ok(futures::StreamExt::filter_map(stream, |item| async move {
            match item {
                Ok(StreamItem::Text(text)) => Some(Ok(text)),
                Ok(StreamItem::Event(_)) => None,
                Err(e) => Some(Err(e)),
            }
        }))
    }

    /// Get configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let _builder = Client::builder();
    }

    #[test]
    fn test_network_config_integration() {
        let network = NetworkConfig::default();
        assert!(!network.is_configured());
    }
}
