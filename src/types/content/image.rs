//! Image source types for content blocks.

use std::path::Path;

use base64::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
    File { file_id: String },
}

impl ImageSource {
    pub fn base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Base64 {
            media_type: media_type.into(),
            data: data.into(),
        }
    }

    pub fn from_url(url: impl Into<String>) -> Self {
        Self::Url { url: url.into() }
    }

    pub fn from_file(file_id: impl Into<String>) -> Self {
        Self::File {
            file_id: file_id.into(),
        }
    }

    pub fn jpeg(data: impl Into<String>) -> Self {
        Self::Base64 {
            media_type: "image/jpeg".into(),
            data: data.into(),
        }
    }

    pub fn png(data: impl Into<String>) -> Self {
        Self::Base64 {
            media_type: "image/png".into(),
            data: data.into(),
        }
    }

    pub fn gif(data: impl Into<String>) -> Self {
        Self::Base64 {
            media_type: "image/gif".into(),
            data: data.into(),
        }
    }

    pub fn webp(data: impl Into<String>) -> Self {
        Self::Base64 {
            media_type: "image/webp".into(),
            data: data.into(),
        }
    }

    pub fn is_base64(&self) -> bool {
        matches!(self, Self::Base64 { .. })
    }

    pub fn is_url(&self) -> bool {
        matches!(self, Self::Url { .. })
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Self::File { .. })
    }

    pub fn file_id(&self) -> Option<&str> {
        match self {
            Self::File { file_id } => Some(file_id),
            _ => None,
        }
    }

    pub fn media_type(&self) -> Option<&str> {
        match self {
            Self::Base64 { media_type, .. } => Some(media_type),
            _ => None,
        }
    }

    pub async fn from_path(path: impl AsRef<Path>) -> crate::Result<Self> {
        let path = path.as_ref();
        let data = tokio::fs::read(path).await.map_err(crate::Error::Io)?;
        let media_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        Ok(Self::Base64 {
            media_type,
            data: BASE64_STANDARD.encode(&data),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_source_variants() {
        let base64 = ImageSource::jpeg("data123");
        assert!(base64.is_base64());
        assert_eq!(base64.media_type(), Some("image/jpeg"));

        let url = ImageSource::from_url("https://example.com/img.png");
        assert!(url.is_url());

        let file = ImageSource::from_file("file_abc123");
        assert!(file.is_file());
        assert_eq!(file.file_id(), Some("file_abc123"));
    }

    #[test]
    fn test_image_source_serialization() {
        let file = ImageSource::from_file("file_xyz");
        let json = serde_json::to_string(&file).unwrap();
        assert!(json.contains("\"type\":\"file\""));
        assert!(json.contains("\"file_id\":\"file_xyz\""));

        let url = ImageSource::from_url("https://example.com/img.png");
        let json = serde_json::to_string(&url).unwrap();
        assert!(json.contains("\"type\":\"url\""));
    }

    #[tokio::test]
    async fn test_image_source_from_path() {
        let dir = tempfile::tempdir().unwrap();
        let png_path = dir.path().join("test.png");

        let png_data: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        tokio::fs::write(&png_path, &png_data).await.unwrap();

        let source = ImageSource::from_path(&png_path).await.unwrap();
        assert!(source.is_base64());
        assert_eq!(source.media_type(), Some("image/png"));

        if let ImageSource::Base64 { data, .. } = &source {
            let decoded = base64::prelude::BASE64_STANDARD.decode(data).unwrap();
            assert_eq!(decoded, png_data);
        } else {
            panic!("Expected Base64 source");
        }
    }

    #[tokio::test]
    async fn test_image_source_from_path_not_found() {
        let result = ImageSource::from_path("/nonexistent/path/image.png").await;
        assert!(result.is_err());
    }
}
