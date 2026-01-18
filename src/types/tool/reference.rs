//! Tool reference types for deferred tool loading.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReference {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub tool_name: String,
}

impl ToolReference {
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            ref_type: "tool_reference".to_string(),
            tool_name: tool_name.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub tool_references: Vec<ToolReference>,
}

impl ToolSearchResult {
    pub fn new(tool_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            result_type: "tool_search_tool_search_result".to_string(),
            tool_references: tool_names.into_iter().map(ToolReference::new).collect(),
        }
    }

    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tool_references.iter().map(|r| r.tool_name.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSearchToolResult {
    pub tool_use_id: String,
    pub content: ToolSearchResultContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolSearchResultContent {
    #[serde(rename = "tool_search_tool_search_result")]
    SearchResult { tool_references: Vec<ToolReference> },
    #[serde(rename = "tool_search_tool_result_error")]
    Error { error_code: ToolSearchErrorCode },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSearchErrorCode {
    TooManyRequests,
    InvalidPattern,
    PatternTooLong,
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_reference_serialization() {
        let reference = ToolReference::new("get_weather");
        let json = serde_json::to_string(&reference).unwrap();
        assert!(json.contains("tool_reference"));
        assert!(json.contains("get_weather"));
    }

    #[test]
    fn test_tool_search_result() {
        let result = ToolSearchResult::new(["tool_a", "tool_b"]);
        assert_eq!(result.tool_references.len(), 2);
        let names: Vec<_> = result.tool_names().collect();
        assert_eq!(names, vec!["tool_a", "tool_b"]);
    }
}
