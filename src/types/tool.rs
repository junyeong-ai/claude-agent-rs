//! Tool-related types.

use serde::{Deserialize, Serialize};

/// Definition of a custom tool for the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON Schema for input parameters
    pub input_schema: serde_json::Value,
}

/// Anthropic built-in web search tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchTool {
    /// Tool type identifier.
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Tool name.
    pub name: String,
    /// Maximum search uses per request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u32>,
    /// Only include results from these domains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    /// Exclude results from these domains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
    /// User location for localized results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<UserLocation>,
}

/// User location for web search localization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLocation {
    /// Location type (e.g., "approximate").
    #[serde(rename = "type")]
    pub location_type: String,
    /// City name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// Region/state name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Country code or name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// IANA timezone identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self {
            tool_type: "web_search_20250305".to_string(),
            name: "web_search".to_string(),
            max_uses: None,
            allowed_domains: None,
            blocked_domains: None,
            user_location: None,
        }
    }
}

impl WebSearchTool {
    /// Create a new web search tool with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum search uses per request.
    pub fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }

    /// Only include results from these domains.
    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = Some(domains);
        self
    }

    /// Exclude results from these domains.
    pub fn with_blocked_domains(mut self, domains: Vec<String>) -> Self {
        self.blocked_domains = Some(domains);
        self
    }

    /// Set user location for localized results.
    pub fn with_user_location(mut self, location: UserLocation) -> Self {
        self.user_location = Some(location);
        self
    }
}

impl UserLocation {
    /// Create approximate location with country.
    pub fn approximate(country: impl Into<String>) -> Self {
        Self {
            location_type: "approximate".to_string(),
            city: None,
            region: None,
            country: Some(country.into()),
            timezone: None,
        }
    }

    /// Set city.
    pub fn with_city(mut self, city: impl Into<String>) -> Self {
        self.city = Some(city.into());
        self
    }

    /// Set region/state.
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set timezone.
    pub fn with_timezone(mut self, timezone: impl Into<String>) -> Self {
        self.timezone = Some(timezone.into());
        self
    }
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
