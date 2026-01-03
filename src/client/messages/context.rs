//! Context management types for message requests.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagement {
    pub edits: Vec<ContextEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContextEdit {
    #[serde(rename = "clear_tool_uses_20250919")]
    ClearToolUses {
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger: Option<ClearTrigger>,
        #[serde(skip_serializing_if = "Option::is_none")]
        keep: Option<KeepConfig>,
        #[serde(skip_serializing_if = "Option::is_none")]
        clear_at_least: Option<ClearConfig>,
        #[serde(skip_serializing_if = "Option::is_none")]
        exclude_tools: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        clear_tool_inputs: Option<bool>,
    },
    #[serde(rename = "clear_thinking_20251015")]
    ClearThinking { keep: KeepThinkingConfig },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClearTrigger {
    InputTokens { value: u32 },
    ToolUses { value: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeepConfig {
    ToolUses { value: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClearConfig {
    InputTokens { value: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeepThinkingConfig {
    ThinkingTurns {
        value: u32,
    },
    #[serde(rename = "all")]
    All,
}

impl ContextManagement {
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    pub fn clear_tool_uses() -> ContextEdit {
        ContextEdit::ClearToolUses {
            trigger: None,
            keep: None,
            clear_at_least: None,
            exclude_tools: None,
            clear_tool_inputs: None,
        }
    }

    pub fn clear_thinking(keep_turns: u32) -> ContextEdit {
        ContextEdit::ClearThinking {
            keep: KeepThinkingConfig::ThinkingTurns { value: keep_turns },
        }
    }

    pub fn clear_thinking_all() -> ContextEdit {
        ContextEdit::ClearThinking {
            keep: KeepThinkingConfig::All,
        }
    }

    pub fn with_edit(mut self, edit: ContextEdit) -> Self {
        self.edits.push(edit);
        self
    }
}

impl Default for ContextManagement {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_management() {
        let mgmt = ContextManagement::new()
            .with_edit(ContextManagement::clear_tool_uses())
            .with_edit(ContextManagement::clear_thinking(3));
        assert_eq!(mgmt.edits.len(), 2);
    }

    #[test]
    fn test_context_edit_serialization() {
        let edit = ContextManagement::clear_tool_uses();
        let json = serde_json::to_string(&edit).unwrap();
        assert!(json.contains("clear_tool_uses_20250919"));

        let edit = ContextManagement::clear_thinking(2);
        let json = serde_json::to_string(&edit).unwrap();
        assert!(json.contains("clear_thinking_20251015"));
        assert!(json.contains("\"type\":\"thinking_turns\""));
        assert!(json.contains("\"value\":2"));

        let edit = ContextManagement::clear_thinking_all();
        let json = serde_json::to_string(&edit).unwrap();
        assert!(json.contains("\"type\":\"all\""));
    }
}
