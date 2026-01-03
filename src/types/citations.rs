//! Citation types for document-based responses.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationsConfig {
    pub enabled: bool,
}

impl CitationsConfig {
    pub const fn enabled() -> Self {
        Self { enabled: true }
    }

    pub const fn disabled() -> Self {
        Self { enabled: false }
    }
}

impl Default for CitationsConfig {
    fn default() -> Self {
        Self::enabled()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Citation {
    CharLocation(CharLocationCitation),
    PageLocation(PageLocationCitation),
    ContentBlockLocation(ContentBlockLocationCitation),
    SearchResultLocation(SearchResultLocationCitation),
    WebSearchResultLocation(WebSearchResultLocationCitation),
}

impl Citation {
    pub fn cited_text(&self) -> &str {
        match self {
            Self::CharLocation(c) => &c.cited_text,
            Self::PageLocation(c) => &c.cited_text,
            Self::ContentBlockLocation(c) => &c.cited_text,
            Self::SearchResultLocation(c) => &c.cited_text,
            Self::WebSearchResultLocation(c) => &c.cited_text,
        }
    }

    pub fn document_title(&self) -> Option<&str> {
        match self {
            Self::CharLocation(c) => c.document_title.as_deref(),
            Self::PageLocation(c) => c.document_title.as_deref(),
            Self::ContentBlockLocation(c) => c.document_title.as_deref(),
            Self::SearchResultLocation(c) => c.title.as_deref(),
            Self::WebSearchResultLocation(c) => c.title.as_deref(),
        }
    }

    pub fn document_index(&self) -> Option<usize> {
        match self {
            Self::CharLocation(c) => Some(c.document_index),
            Self::PageLocation(c) => Some(c.document_index),
            Self::ContentBlockLocation(c) => Some(c.document_index),
            Self::SearchResultLocation(_) | Self::WebSearchResultLocation(_) => None,
        }
    }

    pub fn is_search_result(&self) -> bool {
        matches!(
            self,
            Self::SearchResultLocation(_) | Self::WebSearchResultLocation(_)
        )
    }

    pub fn url(&self) -> Option<&str> {
        match self {
            Self::WebSearchResultLocation(c) => Some(&c.url),
            Self::SearchResultLocation(c) => Some(&c.source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharLocationCitation {
    pub cited_text: String,
    pub document_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub start_char_index: usize,
    pub end_char_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageLocationCitation {
    pub cited_text: String,
    pub document_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub start_page_number: u32,
    pub end_page_number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentBlockLocationCitation {
    pub cited_text: String,
    pub document_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    pub start_block_index: usize,
    pub end_block_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResultLocationCitation {
    pub cited_text: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub search_result_index: usize,
    pub start_block_index: usize,
    pub end_block_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchResultLocationCitation {
    pub cited_text: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypted_index: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_citation_serialization() {
        let citation = Citation::CharLocation(CharLocationCitation {
            cited_text: "test".to_string(),
            document_index: 0,
            document_title: Some("Doc".to_string()),
            start_char_index: 0,
            end_char_index: 4,
        });

        let json = serde_json::to_string(&citation).unwrap();
        assert!(json.contains("char_location"));

        let parsed: Citation = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cited_text(), "test");
    }

    #[test]
    fn test_search_result_citation() {
        let citation = Citation::SearchResultLocation(SearchResultLocationCitation {
            cited_text: "search text".to_string(),
            source: "https://example.com".to_string(),
            title: Some("Example".to_string()),
            search_result_index: 0,
            start_block_index: 0,
            end_block_index: 1,
        });

        assert!(citation.is_search_result());
        assert!(citation.document_index().is_none());
    }
}
