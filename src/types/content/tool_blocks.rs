//! Tool use and result block types.

use serde::{Deserialize, Serialize};

use super::image::ImageSource;
use crate::types::search::SearchResultBlock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseBlock {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    pub tool_use_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ToolResultContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Blocks(Vec<ToolResultContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContentBlock {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    #[serde(rename = "search_result")]
    SearchResult(SearchResultBlock),
}

impl ToolResultBlock {
    pub fn success(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Text(content.into())),
            is_error: None,
        }
    }

    pub fn error(tool_use_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Text(message.into())),
            is_error: Some(true),
        }
    }

    pub fn empty(tool_use_id: impl Into<String>) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: None,
            is_error: None,
        }
    }

    pub fn success_blocks(
        tool_use_id: impl Into<String>,
        blocks: Vec<crate::types::ToolOutputBlock>,
    ) -> Self {
        use crate::types::ToolOutputBlock;

        let content_blocks: Vec<ToolResultContentBlock> = blocks
            .into_iter()
            .map(|block| match block {
                ToolOutputBlock::Text { text } => ToolResultContentBlock::Text { text },
                ToolOutputBlock::Image { data, media_type } => ToolResultContentBlock::Image {
                    source: ImageSource::Base64 { media_type, data },
                },
                ToolOutputBlock::SearchResult(sr) => ToolResultContentBlock::SearchResult(sr),
            })
            .collect();

        Self {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Blocks(content_blocks)),
            is_error: None,
        }
    }

    pub fn search_results(tool_use_id: impl Into<String>, results: Vec<SearchResultBlock>) -> Self {
        let content_blocks: Vec<ToolResultContentBlock> = results
            .into_iter()
            .map(ToolResultContentBlock::SearchResult)
            .collect();

        Self {
            tool_use_id: tool_use_id.into(),
            content: Some(ToolResultContent::Blocks(content_blocks)),
            is_error: None,
        }
    }

    pub fn from_tool_result(tool_use_id: &str, result: &crate::types::tool::ToolResult) -> Self {
        use crate::types::tool::ToolOutput;
        match &result.output {
            ToolOutput::Success(content) => Self::success(tool_use_id, content.clone()),
            ToolOutput::SuccessBlocks(blocks) => Self::success_blocks(tool_use_id, blocks.clone()),
            ToolOutput::Error(e) => Self::error(tool_use_id, e.to_string()),
            ToolOutput::Empty => Self::empty(tool_use_id),
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

    #[test]
    fn test_tool_result_search_results() {
        let results = vec![SearchResultBlock::new(
            "https://example.com",
            "Title",
            "content",
        )];
        let tool_result = ToolResultBlock::search_results("tool_123", results);

        if let Some(ToolResultContent::Blocks(blocks)) = tool_result.content {
            assert_eq!(blocks.len(), 1);
            assert!(matches!(blocks[0], ToolResultContentBlock::SearchResult(_)));
        } else {
            panic!("Expected blocks content");
        }
    }
}
