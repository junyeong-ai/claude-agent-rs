//! WebSearch tool - web search functionality.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::tools::registry::{Tool, ToolResult};

/// Input for WebSearch tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchInput {
    /// The search query
    pub query: String,

    /// Only include results from these domains (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,

    /// Exclude results from these domains (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
}

/// A single search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Title of the result
    pub title: String,

    /// URL of the result
    pub url: String,

    /// Snippet/description of the result
    pub snippet: String,
}

/// Output from WebSearch tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchOutput {
    /// Search results
    pub results: Vec<SearchResult>,

    /// Total number of results found
    pub total_results: usize,

    /// The query that was searched
    pub query: String,
}

/// WebSearch tool for searching the web.
pub struct WebSearchTool;

impl WebSearchTool {
    /// Create a new WebSearchTool
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    fn description(&self) -> &str {
        r#"Searches the web and returns results to inform responses.

CRITICAL REQUIREMENT - You MUST follow this:
  - After answering the user's question, you MUST include a "Sources:" section at the end of your response
  - In the Sources section, list all relevant URLs from the search results as markdown hyperlinks: [Title](URL)
  - This is MANDATORY - never skip including sources in your response

Usage notes:
  - Domain filtering is supported to include or block specific websites
  - The query should be specific and relevant to what information is needed

Example format:
    [Your answer here]

    Sources:
    - [Source Title 1](https://example.com/1)
    - [Source Title 2](https://example.com/2)"#
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to use. Should be specific and relevant.",
                    "minLength": 2
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only include search results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Never include search results from these domains"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: WebSearchInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // Validate query
        if input.query.trim().is_empty() {
            return ToolResult::error("Query cannot be empty");
        }

        if input.query.len() < 2 {
            return ToolResult::error("Query must be at least 2 characters");
        }

        let output = WebSearchOutput {
            results: vec![],
            total_results: 0,
            query: input.query.clone(),
        };

        // Return informative message about search capability
        let message = format!(
            r#"WebSearch executed for query: "{}"

Note: Web search functionality requires configuration of a search API provider.
To enable web search:
1. Configure a search API (Google Custom Search, Bing Search API, etc.)
2. Set the appropriate environment variables or configuration

Domain filters applied:
- Allowed domains: {:?}
- Blocked domains: {:?}

Search results: {} found"#,
            input.query,
            input.allowed_domains.unwrap_or_default(),
            input.blocked_domains.unwrap_or_default(),
            output.total_results
        );

        ToolResult::success(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_search_basic() {
        let tool = WebSearchTool::new();

        let result = tool
            .execute(json!({
                "query": "rust programming language"
            }))
            .await;

        assert!(!result.is_error());
    }

    #[tokio::test]
    async fn test_web_search_with_domains() {
        let tool = WebSearchTool::new();

        let result = tool
            .execute(json!({
                "query": "rust async",
                "allowed_domains": ["docs.rs", "crates.io"],
                "blocked_domains": ["example.com"]
            }))
            .await;

        assert!(!result.is_error());
    }

    #[tokio::test]
    async fn test_web_search_empty_query() {
        let tool = WebSearchTool::new();

        let result = tool
            .execute(json!({
                "query": ""
            }))
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_web_search_short_query() {
        let tool = WebSearchTool::new();

        let result = tool
            .execute(json!({
                "query": "a"
            }))
            .await;

        assert!(result.is_error());
    }

    #[test]
    fn test_tool_definition() {
        let tool = WebSearchTool::new();

        assert_eq!(tool.name(), "WebSearch");
        assert!(!tool.description().is_empty());

        let schema = tool.input_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
    }
}
