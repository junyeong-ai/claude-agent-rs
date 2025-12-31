//! Tool-related types.

use serde::{Deserialize, Serialize};

/// Definition of a tool for the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// Input for a tool execution
#[derive(Debug, Clone)]
pub struct ToolInput {
    /// Tool use ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Input parameters
    pub input: serde_json::Value,
}

/// Output from a tool execution
#[derive(Debug, Clone)]
pub enum ToolOutput {
    /// Successful result with content
    Success(String),
    /// Successful result with multiple content blocks
    SuccessBlocks(Vec<ToolOutputBlock>),
    /// Error result
    Error(String),
    /// Empty result (success, no content)
    Empty,
}

/// A content block in tool output
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolOutputBlock {
    /// Text content
    Text {
        /// The text
        text: String,
    },
    /// Image content
    Image {
        /// Base64 encoded image
        data: String,
        /// Media type
        media_type: String,
    },
}

impl ToolOutput {
    /// Create a success output
    pub fn success(content: impl Into<String>) -> Self {
        Self::Success(content.into())
    }

    /// Create an error output
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }

    /// Create an empty success output
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

impl From<String> for ToolOutput {
    fn from(s: String) -> Self {
        Self::Success(s)
    }
}

impl From<&str> for ToolOutput {
    fn from(s: &str) -> Self {
        Self::Success(s.to_string())
    }
}

impl<T, E> From<Result<T, E>> for ToolOutput
where
    T: Into<String>,
    E: std::fmt::Display,
{
    fn from(result: Result<T, E>) -> Self {
        match result {
            Ok(content) => Self::Success(content.into()),
            Err(e) => Self::Error(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_output_from_result() {
        let ok: Result<&str, &str> = Ok("success");
        let output: ToolOutput = ok.into();
        assert!(!output.is_error());

        let err: Result<&str, &str> = Err("failed");
        let output: ToolOutput = err.into();
        assert!(output.is_error());
    }
}
