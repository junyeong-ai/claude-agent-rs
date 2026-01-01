//! Model configuration with provider-specific defaults.

use std::env;

/// Default primary model identifier.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
/// Default small/fast model identifier.
pub const DEFAULT_SMALL_MODEL: &str = "claude-haiku-4-5-20251001";

/// Bedrock primary model identifier (with inference profile prefix).
pub const BEDROCK_MODEL: &str = "us.anthropic.claude-3-7-sonnet-20250219-v1:0";
/// Bedrock small/fast model identifier.
pub const BEDROCK_SMALL_MODEL: &str = "us.anthropic.claude-haiku-4-5-20251001-v1:0";

/// Vertex AI primary model identifier.
pub const VERTEX_MODEL: &str = "claude-sonnet-4-5@20250929";
/// Vertex AI small/fast model identifier.
pub const VERTEX_SMALL_MODEL: &str = "claude-haiku-4-5@20251001";

/// Model configuration for primary and small models.
#[derive(Clone, Debug)]
pub struct ModelConfig {
    /// Primary model
    pub primary: String,
    /// Small/fast model
    pub small: String,
}

impl ModelConfig {
    /// Create from environment variables.
    pub fn from_env() -> Self {
        Self {
            primary: env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
            small: env::var("ANTHROPIC_SMALL_FAST_MODEL")
                .unwrap_or_else(|_| DEFAULT_SMALL_MODEL.to_string()),
        }
    }

    /// Create with default Bedrock models.
    pub fn for_bedrock() -> Self {
        Self {
            primary: env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| BEDROCK_MODEL.to_string()),
            small: env::var("ANTHROPIC_SMALL_FAST_MODEL")
                .unwrap_or_else(|_| BEDROCK_SMALL_MODEL.to_string()),
        }
    }

    /// Create with default Vertex models.
    pub fn for_vertex() -> Self {
        Self {
            primary: env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| VERTEX_MODEL.to_string()),
            small: env::var("ANTHROPIC_SMALL_FAST_MODEL")
                .unwrap_or_else(|_| VERTEX_SMALL_MODEL.to_string()),
        }
    }

    /// Set primary model.
    pub fn with_primary(mut self, model: impl Into<String>) -> Self {
        self.primary = model.into();
        self
    }

    /// Set small model.
    pub fn with_small(mut self, model: impl Into<String>) -> Self {
        self.small = model.into();
        self
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_defaults() {
        let config = ModelConfig::default();
        assert!(!config.primary.is_empty());
        assert!(!config.small.is_empty());
    }

    #[test]
    fn test_bedrock_models() {
        let config = ModelConfig::for_bedrock();
        assert!(config.primary.contains("anthropic"));
    }

    #[test]
    fn test_vertex_models() {
        let config = ModelConfig::for_vertex();
        assert!(config.primary.contains("@"));
    }
}
