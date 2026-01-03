//! Configuration types for message requests.

use serde::{Deserialize, Serialize};

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
            budget_tokens: Some(budget_tokens),
        }
    }

    pub fn disabled() -> Self {
        Self {
            thinking_type: ThinkingType::Disabled,
            budget_tokens: None,
        }
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
        assert_eq!(config.budget_tokens, Some(10000));
    }

    #[test]
    fn test_thinking_config_disabled() {
        let config = ThinkingConfig::disabled();
        assert_eq!(config.thinking_type, ThinkingType::Disabled);
        assert_eq!(config.budget_tokens, None);
    }

    #[test]
    fn test_thinking_config_serialization() {
        let config = ThinkingConfig::enabled(5000);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"enabled\""));
        assert!(json.contains("\"budget_tokens\":5000"));
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
        let effort = EffortLevel::Low;
        let json = serde_json::to_string(&effort).unwrap();
        assert_eq!(json, "\"low\"");

        let effort = EffortLevel::Medium;
        let json = serde_json::to_string(&effort).unwrap();
        assert_eq!(json, "\"medium\"");

        let effort = EffortLevel::High;
        let json = serde_json::to_string(&effort).unwrap();
        assert_eq!(json, "\"high\"");
    }

    #[test]
    fn test_output_config_serialization() {
        let config = OutputConfig::with_effort(EffortLevel::High);
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"effort\":\"high\""));
    }

    #[test]
    fn test_tool_choice_auto() {
        let choice = ToolChoice::Auto;
        let json = serde_json::to_string(&choice).unwrap();
        assert_eq!(json, r#"{"type":"auto"}"#);
    }

    #[test]
    fn test_tool_choice_any() {
        let choice = ToolChoice::Any;
        let json = serde_json::to_string(&choice).unwrap();
        assert_eq!(json, r#"{"type":"any"}"#);
    }

    #[test]
    fn test_tool_choice_none() {
        let choice = ToolChoice::None;
        let json = serde_json::to_string(&choice).unwrap();
        assert_eq!(json, r#"{"type":"none"}"#);
    }

    #[test]
    fn test_tool_choice_tool() {
        let choice = ToolChoice::tool("Bash");
        let json = serde_json::to_string(&choice).unwrap();
        assert_eq!(json, r#"{"type":"tool","name":"Bash"}"#);
    }
}
