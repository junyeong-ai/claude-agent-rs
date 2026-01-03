//! Provider adapters for different cloud platforms.

mod anthropic;
#[cfg(any(feature = "aws", feature = "gcp", feature = "azure"))]
mod base;
mod config;
mod request;
mod traits;

#[cfg(any(feature = "aws", feature = "gcp", feature = "azure"))]
mod token_cache;

#[cfg(feature = "aws")]
mod bedrock;
#[cfg(feature = "azure")]
mod foundry;
#[cfg(feature = "gcp")]
mod vertex;

pub use anthropic::AnthropicAdapter;
pub use config::{
    BetaConfig, BetaFeature, DEFAULT_MODEL, DEFAULT_REASONING_MODEL, DEFAULT_SMALL_MODEL,
    FRONTIER_MODEL, ModelConfig, ModelType, ProviderConfig,
};
pub use traits::ProviderAdapter;

#[cfg(feature = "aws")]
pub use bedrock::BedrockAdapter;
#[cfg(feature = "azure")]
pub use foundry::FoundryAdapter;
#[cfg(feature = "gcp")]
pub use vertex::VertexAdapter;

use crate::Result;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CloudProvider {
    #[default]
    Anthropic,
    #[cfg(feature = "aws")]
    Bedrock,
    #[cfg(feature = "gcp")]
    Vertex,
    #[cfg(feature = "azure")]
    Foundry,
}

impl CloudProvider {
    pub fn from_env() -> Self {
        #[cfg(feature = "aws")]
        if std::env::var("CLAUDE_CODE_USE_BEDROCK").is_ok() {
            return Self::Bedrock;
        }
        #[cfg(feature = "gcp")]
        if std::env::var("CLAUDE_CODE_USE_VERTEX").is_ok() {
            return Self::Vertex;
        }
        #[cfg(feature = "azure")]
        if std::env::var("CLAUDE_CODE_USE_FOUNDRY").is_ok() {
            return Self::Foundry;
        }
        Self::Anthropic
    }

    pub fn default_models(&self) -> ModelConfig {
        match self {
            Self::Anthropic => ModelConfig::anthropic(),
            #[cfg(feature = "aws")]
            Self::Bedrock => ModelConfig::bedrock(),
            #[cfg(feature = "gcp")]
            Self::Vertex => ModelConfig::vertex(),
            #[cfg(feature = "azure")]
            Self::Foundry => ModelConfig::foundry(),
        }
    }
}

pub async fn create_adapter(
    provider: CloudProvider,
    config: ProviderConfig,
) -> Result<Box<dyn ProviderAdapter>> {
    match provider {
        CloudProvider::Anthropic => Ok(Box::new(AnthropicAdapter::new(config))),
        #[cfg(feature = "aws")]
        CloudProvider::Bedrock => Ok(Box::new(BedrockAdapter::from_env(config).await?)),
        #[cfg(feature = "gcp")]
        CloudProvider::Vertex => Ok(Box::new(VertexAdapter::from_env(config).await?)),
        #[cfg(feature = "azure")]
        CloudProvider::Foundry => Ok(Box::new(FoundryAdapter::from_env(config).await?)),
    }
}
