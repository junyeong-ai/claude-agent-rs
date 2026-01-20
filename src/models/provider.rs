use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    #[default]
    Anthropic,
    Bedrock,
    Vertex,
    Foundry,
}

impl CloudProvider {
    pub fn from_env() -> Self {
        if env::var("CLAUDE_CODE_USE_BEDROCK").is_ok() {
            Self::Bedrock
        } else if env::var("CLAUDE_CODE_USE_VERTEX").is_ok() {
            Self::Vertex
        } else if env::var("CLAUDE_CODE_USE_FOUNDRY").is_ok() {
            Self::Foundry
        } else {
            Self::Anthropic
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderIds {
    pub anthropic: Option<String>,
    pub bedrock: Option<String>,
    pub vertex: Option<String>,
    pub foundry: Option<String>,
}

impl ProviderIds {
    pub fn for_provider(&self, provider: CloudProvider) -> Option<&str> {
        match provider {
            CloudProvider::Anthropic => self.anthropic.as_deref(),
            CloudProvider::Bedrock => self.bedrock.as_deref(),
            CloudProvider::Vertex => self.vertex.as_deref(),
            CloudProvider::Foundry => self.foundry.as_deref(),
        }
    }
}
