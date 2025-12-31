//! Message types for the Claude API.

use serde::{Deserialize, Serialize};

use super::ContentBlock;

/// Role of a message participant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// User message
    User,
    /// Assistant (Claude) message
    Assistant,
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: Role,
    /// Content of the message
    pub content: Vec<ContentBlock>,
}

impl Message {
    /// Create a new user message with text content
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create a new assistant message with text content
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }

    /// Create a user message with tool results
    pub fn tool_results(results: Vec<super::ToolResultBlock>) -> Self {
        Self {
            role: Role::User,
            content: results.into_iter().map(ContentBlock::ToolResult).collect(),
        }
    }

    /// Get the text content of the message (concatenated)
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Check if the message contains tool use blocks
    pub fn has_tool_use(&self) -> bool {
        self.content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }))
    }

    /// Extract all tool use blocks from the message
    pub fn tool_uses(&self) -> Vec<&super::ToolUseBlock> {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse(tool_use) => Some(tool_use),
                _ => None,
            })
            .collect()
    }
}

/// System prompt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    /// Simple text system prompt
    Text(String),
    /// Structured system prompt with cache control
    Blocks(Vec<SystemBlock>),
}

/// A block in a structured system prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBlock {
    /// Type of the block (always "text" for now)
    #[serde(rename = "type")]
    pub block_type: String,
    /// Text content
    pub text: String,
    /// Optional cache control
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

/// Cache control for prompt caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    /// Cache type
    #[serde(rename = "type")]
    pub cache_type: String,
}

impl SystemPrompt {
    /// Create a simple text system prompt
    pub fn text(prompt: impl Into<String>) -> Self {
        Self::Text(prompt.into())
    }

    /// Create a system prompt with caching enabled
    pub fn cached(prompt: impl Into<String>) -> Self {
        Self::Blocks(vec![SystemBlock {
            block_type: "text".to_string(),
            text: prompt.into(),
            cache_control: Some(CacheControl {
                cache_type: "ephemeral".to_string(),
            }),
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.text(), "Hello");
    }

    #[test]
    fn test_assistant_message() {
        let msg = Message::assistant("Hi there!");
        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.text(), "Hi there!");
    }
}
