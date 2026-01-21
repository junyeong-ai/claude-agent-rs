//! Server-side tool types (web_search, web_fetch).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolUseBlock {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub error_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchToolResultBlock {
    pub tool_use_id: String,
    pub content: WebSearchToolResultContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebSearchToolResultContent {
    Results(Vec<WebSearchResultItem>),
    Error(ServerToolError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResultItem {
    #[serde(rename = "type")]
    pub result_type: String,
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_age: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchToolResultBlock {
    pub tool_use_id: String,
    pub content: WebFetchToolResultContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebFetchToolResultContent {
    Result(WebFetchResultItem),
    Error(ServerToolError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchResultItem {
    #[serde(rename = "type")]
    pub result_type: String,
    pub url: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieved_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::types::ContentBlock;

    #[test]
    fn test_server_tool_use_block_parsing() {
        let json = r#"{
            "type": "server_tool_use",
            "id": "srvtoolu_01WYG3ziw53XMcoyKL4XcZmE",
            "name": "web_search",
            "input": {"query": "claude shannon birth date"}
        }"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(block.is_server_tool_use());
        let stu = block.as_server_tool_use().unwrap();
        assert_eq!(stu.name, "web_search");
        assert_eq!(stu.id, "srvtoolu_01WYG3ziw53XMcoyKL4XcZmE");
    }

    #[test]
    fn test_web_search_tool_result_parsing() {
        let json = r#"{
            "type": "web_search_tool_result",
            "tool_use_id": "srvtoolu_01WYG3ziw53XMcoyKL4XcZmE",
            "content": [
                {
                    "type": "web_search_result",
                    "url": "https://en.wikipedia.org/wiki/Claude_Shannon",
                    "title": "Claude Shannon - Wikipedia",
                    "encrypted_content": "EqgfCioIARgB...",
                    "page_age": "April 30, 2025"
                }
            ]
        }"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(block.is_web_search_result());
        let wsr = block.as_web_search_result().unwrap();
        assert_eq!(wsr.tool_use_id, "srvtoolu_01WYG3ziw53XMcoyKL4XcZmE");
    }

    #[test]
    fn test_web_fetch_tool_result_parsing() {
        let json = r#"{
            "type": "web_fetch_tool_result",
            "tool_use_id": "srvtoolu_01234567890abcdef",
            "content": {
                "type": "web_fetch_result",
                "url": "https://example.com/article",
                "content": {"type": "document", "data": "article content..."},
                "retrieved_at": "2025-08-25T10:30:00Z"
            }
        }"#;
        let block: ContentBlock = serde_json::from_str(json).unwrap();
        assert!(block.is_web_fetch_result());
        let wfr = block.as_web_fetch_result().unwrap();
        assert_eq!(wfr.tool_use_id, "srvtoolu_01234567890abcdef");
    }
}
