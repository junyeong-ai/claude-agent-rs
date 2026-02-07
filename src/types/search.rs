//! Search result content block for web search citations.

use serde::{Deserialize, Serialize};

use super::citations::CitationsConfig;
use super::message::CacheControl;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchResultContentBlock {
    Text { text: String },
}

impl SearchResultContentBlock {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            text: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResultBlock {
    pub source: String,
    pub title: String,
    pub content: Vec<SearchResultContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<CitationsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl SearchResultBlock {
    pub fn new(
        source: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            title: title.into(),
            content: vec![SearchResultContentBlock::Text {
                text: content.into(),
            }],
            citations: None,
            cache_control: None,
        }
    }

    pub fn blocks(
        source: impl Into<String>,
        title: impl Into<String>,
        blocks: Vec<SearchResultContentBlock>,
    ) -> Self {
        Self {
            source: source.into(),
            title: title.into(),
            content: blocks,
            citations: None,
            cache_control: None,
        }
    }

    pub fn add_text(mut self, text: impl Into<String>) -> Self {
        self.content
            .push(SearchResultContentBlock::Text { text: text.into() });
        self
    }

    pub fn citations(mut self, enabled: bool) -> Self {
        self.citations = Some(if enabled {
            CitationsConfig::enabled()
        } else {
            CitationsConfig::disabled()
        });
        self
    }

    pub fn without_citations(mut self) -> Self {
        self.citations = Some(CitationsConfig::disabled());
        self
    }

    pub fn cache_control(mut self, cache_control: CacheControl) -> Self {
        self.cache_control = Some(cache_control);
        self
    }

    pub fn cached(mut self) -> Self {
        self.cache_control = Some(CacheControl::ephemeral());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_new() {
        let result = SearchResultBlock::new("https://example.com", "Example Page", "Some content");

        assert_eq!(result.source, "https://example.com");
        assert_eq!(result.title, "Example Page");
        assert_eq!(result.content.len(), 1);
        assert!(result.citations.is_none());
    }

    #[test]
    fn test_search_result_with_citations() {
        let result = SearchResultBlock::new("https://example.com", "Example Page", "Content")
            .citations(true);

        assert!(result.citations.is_some());
        assert!(result.citations.unwrap().enabled);
    }

    #[test]
    fn test_search_result_multiple_blocks() {
        let result = SearchResultBlock::new("https://example.com", "Title", "First block")
            .add_text("Second block")
            .add_text("Third block");

        assert_eq!(result.content.len(), 3);
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResultBlock::new("https://example.com", "Title", "content");

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("https://example.com"));
        assert!(json.contains("content"));
        assert!(json.contains("\"title\":\"Title\""));
    }
}
