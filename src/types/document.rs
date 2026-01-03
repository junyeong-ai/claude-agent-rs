//! Document content block types for citations.

use serde::{Deserialize, Serialize};

use super::citations::CitationsConfig;
use super::message::CacheControl;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentSource {
    Text { media_type: String, data: String },
    Base64 { media_type: String, data: String },
    Content { content: Vec<DocumentContentBlock> },
    File { file_id: String },
    Url { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentContentBlock {
    Text { text: String },
}

impl DocumentContentBlock {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text {
            text: content.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentBlock {
    pub source: DocumentSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<CitationsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl DocumentBlock {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            source: DocumentSource::Text {
                media_type: "text/plain".to_string(),
                data: content.into(),
            },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    pub fn html(content: impl Into<String>) -> Self {
        Self {
            source: DocumentSource::Text {
                media_type: "text/html".to_string(),
                data: content.into(),
            },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    pub fn markdown(content: impl Into<String>) -> Self {
        Self {
            source: DocumentSource::Text {
                media_type: "text/markdown".to_string(),
                data: content.into(),
            },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    pub fn pdf_base64(data: impl Into<String>) -> Self {
        Self {
            source: DocumentSource::Base64 {
                media_type: "application/pdf".to_string(),
                data: data.into(),
            },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            source: DocumentSource::Url { url: url.into() },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    pub fn from_file(file_id: impl Into<String>) -> Self {
        Self {
            source: DocumentSource::File {
                file_id: file_id.into(),
            },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    pub fn structured(blocks: Vec<DocumentContentBlock>) -> Self {
        Self {
            source: DocumentSource::Content { content: blocks },
            title: None,
            context: None,
            citations: None,
            cache_control: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_citations(mut self, enabled: bool) -> Self {
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

    pub fn with_cache_control(mut self, cache_control: CacheControl) -> Self {
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
    fn test_document_text() {
        let doc = DocumentBlock::text("Hello world")
            .with_title("Test Doc")
            .cached();

        assert!(doc.title.is_some());
        assert!(doc.cache_control.is_some());
        assert!(matches!(doc.source, DocumentSource::Text { .. }));
    }

    #[test]
    fn test_document_structured() {
        let doc = DocumentBlock::structured(vec![
            DocumentContentBlock::text("Block 1"),
            DocumentContentBlock::text("Block 2"),
        ]);

        if let DocumentSource::Content { content } = &doc.source {
            assert_eq!(content.len(), 2);
        } else {
            panic!("Expected Content source");
        }
    }

    #[test]
    fn test_document_serialization() {
        let doc = DocumentBlock::text("test content").with_title("Title");
        let json = serde_json::to_string(&doc).unwrap();

        assert!(json.contains("text/plain"));
        assert!(json.contains("test content"));
    }
}
