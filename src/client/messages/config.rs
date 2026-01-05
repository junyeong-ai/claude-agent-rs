//! Configuration types for message requests.

use serde::{Deserialize, Serialize};

pub const MIN_THINKING_BUDGET: u32 = 1024;
pub const DEFAULT_MAX_TOKENS: u32 = 8192;
pub const MAX_TOKENS_128K: u32 = 128_000;
pub const MIN_MAX_TOKENS: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenValidationError {
    ThinkingBudgetExceedsMaxTokens { budget: u32, max_tokens: u32 },
    MaxTokensTooLow { min: u32, actual: u32 },
    MaxTokensTooHigh { max: u32, actual: u32 },
}

impl std::fmt::Display for TokenValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ThinkingBudgetExceedsMaxTokens { budget, max_tokens } => {
                write!(
                    f,
                    "thinking budget_tokens ({budget}) must be < max_tokens ({max_tokens})"
                )
            }
            Self::MaxTokensTooLow { min, actual } => {
                write!(f, "max_tokens ({actual}) must be >= {min}")
            }
            Self::MaxTokensTooHigh { max, actual } => {
                write!(f, "max_tokens ({actual}) exceeds maximum allowed ({max})")
            }
        }
    }
}

impl std::error::Error for TokenValidationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
    None,
}

impl ToolChoice {
    pub fn tool(name: impl Into<String>) -> Self {
        Self::Tool { name: name.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<EffortLevel>,
}

impl OutputConfig {
    pub fn with_effort(level: EffortLevel) -> Self {
        Self {
            effort: Some(level),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: ThinkingType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingType {
    Enabled,
    Disabled,
}

impl ThinkingConfig {
    pub fn enabled(budget_tokens: u32) -> Self {
        Self {
            thinking_type: ThinkingType::Enabled,
            budget_tokens: Some(budget_tokens.max(MIN_THINKING_BUDGET)),
        }
    }

    pub fn disabled() -> Self {
        Self {
            thinking_type: ThinkingType::Disabled,
            budget_tokens: None,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.thinking_type == ThinkingType::Enabled
    }

    pub fn budget(&self) -> Option<u32> {
        self.budget_tokens
    }

    pub fn validate_against_max_tokens(&self, max_tokens: u32) -> Result<(), TokenValidationError> {
        if let Some(budget) = self.budget_tokens
            && budget >= max_tokens
        {
            return Err(TokenValidationError::ThinkingBudgetExceedsMaxTokens {
                budget,
                max_tokens,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputFormat {
    JsonSchema {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        schema: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
    },
}

impl OutputFormat {
    pub fn json_schema(schema: serde_json::Value) -> Self {
        Self::JsonSchema {
            name: None,
            schema,
            description: None,
        }
    }

    pub fn json_schema_named(name: impl Into<String>, schema: serde_json::Value) -> Self {
        Self::JsonSchema {
            name: Some(name.into()),
            schema,
            description: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_config_enabled() {
        let config = ThinkingConfig::enabled(10000);
        assert_eq!(config.thinking_type, ThinkingType::Enabled);
        assert_eq!(config.budget(), Some(10000));
        assert!(config.is_enabled());
    }

    #[test]
    fn test_thinking_config_enabled_auto_clamp() {
        let config = ThinkingConfig::enabled(500);
        assert!(config.is_enabled());
        assert_eq!(config.budget(), Some(MIN_THINKING_BUDGET));

        let config = ThinkingConfig::enabled(MIN_THINKING_BUDGET);
        assert_eq!(config.budget(), Some(MIN_THINKING_BUDGET));
    }

    #[test]
    fn test_thinking_config_disabled() {
        let config = ThinkingConfig::disabled();
        assert_eq!(config.thinking_type, ThinkingType::Disabled);
        assert_eq!(config.budget(), None);
        assert!(!config.is_enabled());
    }

    #[test]
    fn test_thinking_config_validate_against_max_tokens() {
        let config = ThinkingConfig::enabled(2000);
        assert!(config.validate_against_max_tokens(4000).is_ok());
        assert!(config.validate_against_max_tokens(2000).is_err());
        assert!(config.validate_against_max_tokens(1000).is_err());
    }

    #[test]
    fn test_thinking_config_serialization() {
        let config = ThinkingConfig::enabled(5000);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"enabled\""));
        assert!(json.contains("\"budget_tokens\":5000"));
    }

    #[test]
    fn test_token_validation_error_display() {
        let err = TokenValidationError::ThinkingBudgetExceedsMaxTokens {
            budget: 5000,
            max_tokens: 4000,
        };
        assert_eq!(
            err.to_string(),
            "thinking budget_tokens (5000) must be < max_tokens (4000)"
        );

        let err = TokenValidationError::MaxTokensTooLow { min: 1, actual: 0 };
        assert_eq!(err.to_string(), "max_tokens (0) must be >= 1");

        let err = TokenValidationError::MaxTokensTooHigh {
            max: 128_000,
            actual: 200_000,
        };
        assert_eq!(
            err.to_string(),
            "max_tokens (200000) exceeds maximum allowed (128000)"
        );
    }

    #[test]
    fn test_output_format_json_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"}
            }
        });
        let format = OutputFormat::json_schema(schema);
        let json = serde_json::to_string(&format).unwrap();
        assert!(json.contains("\"type\":\"json_schema\""));
        assert!(json.contains("\"schema\""));
    }

    #[test]
    fn test_output_format_named_schema() {
        let schema = serde_json::json!({"type": "string"});
        let format = OutputFormat::json_schema_named("PersonName", schema);
        let json = serde_json::to_string(&format).unwrap();
        assert!(json.contains("\"name\":\"PersonName\""));
    }

    #[test]
    fn test_effort_level_serialization() {
        assert_eq!(serde_json::to_string(&EffortLevel::Low).unwrap(), "\"low\"");
        assert_eq!(
            serde_json::to_string(&EffortLevel::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(
            serde_json::to_string(&EffortLevel::High).unwrap(),
            "\"high\""
        );
    }

    #[test]
    fn test_output_config_serialization() {
        let config = OutputConfig::with_effort(EffortLevel::High);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"effort\":\"high\""));
    }

    #[test]
    fn test_tool_choice_serialization() {
        assert_eq!(
            serde_json::to_string(&ToolChoice::Auto).unwrap(),
            r#"{"type":"auto"}"#
        );
        assert_eq!(
            serde_json::to_string(&ToolChoice::Any).unwrap(),
            r#"{"type":"any"}"#
        );
        assert_eq!(
            serde_json::to_string(&ToolChoice::None).unwrap(),
            r#"{"type":"none"}"#
        );
        assert_eq!(
            serde_json::to_string(&ToolChoice::tool("Bash")).unwrap(),
            r#"{"type":"tool","name":"Bash"}"#
        );
    }

    #[test]
    fn test_token_constants() {
        assert_eq!(MIN_THINKING_BUDGET, 1024);
        assert_eq!(DEFAULT_MAX_TOKENS, 8192);
        assert_eq!(MAX_TOKENS_128K, 128_000);
        assert_eq!(MIN_MAX_TOKENS, 1);
        assert!(MIN_THINKING_BUDGET < DEFAULT_MAX_TOKENS);
    }
}
