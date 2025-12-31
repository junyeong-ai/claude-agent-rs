//! Content block types for messages.

use serde::{Deserialize, Serialize};

/// A content block within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    Image {
        /// Image source
        source: ImageSource,
    },
    /// Tool use request from Claude
    #[serde(rename = "tool_use")]
    ToolUse(ToolUseBlock),
    /// Tool result from execution
    #[serde(rename = "tool_result")]
    ToolResult(ToolResultBlock),
}

/// Image source for image content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64 encoded image
    Base64 {
        /// Media type (e.g., "image/png")
        media_type: String,
        /// Base64 encoded data
        data: String,
    },
    /// URL reference
    Url {
        /// Image URL
        url: String,
    },
}

/// A tool use block from Claude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseBlock {
    /// Unique identifier for this tool use
    pub id: String,
    /// Name of the tool to use
    pub name: String,
    /// Input parameters for the tool
    pub input: serde_json::Value,
}

/// A tool result block to return to Claude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    /// Tool use ID this is a result for
    pub tool_use_id: String,
    /// Result content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ToolResultContent>,
    /// Whether the tool execution was an error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content of a tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Simple text result
    Text(String),
    /// Multiple content blocks
    Blocks(Vec<ToolResultContentBlock>),
}

/// A content block within a tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentBlock {
    /// Text content
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    Image {
        /// Image source
        source: ImageSource,
    },
}

impl ToolResultBlock {
    /// Create a successful tool result with text content
    pub fn success(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Text(content.into())),
            is_error: None,
        }
    }

    /// Create an error tool result
    pub fn error(tool_use_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Text(message.into())),
            is_error: Some(true),
        }
    }

    /// Create an empty success result
    pub fn empty(tool_use_id: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: None,
            is_error: None,
        }
    }
}

/// Text block helper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    /// Text content
    pub text: String,
}

impl From<&str> for ContentBlock {
    fn from(text: &str) -> Self {
        ContentBlock::Text {
            text: text.to_string(),
        }
    }
}

impl From<String> for ContentBlock {
    fn from(text: String) -> Self {
        ContentBlock::Text { text }
    }
}

impl ContentBlock {
    /// Create a text content block
    pub fn text(text: impl Into<String>) -> Self {
        ContentBlock::Text { text: text.into() }
    }

    /// Get text content if this is a text block
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_success() {
        let result = ToolResultBlock::success("tool_123", "Operation completed");
        assert_eq!(result.tool_use_id, "tool_123");
        assert!(result.is_error.is_none());
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResultBlock::error("tool_456", "File not found");
        assert_eq!(result.tool_use_id, "tool_456");
        assert_eq!(result.is_error, Some(true));
    }
}
