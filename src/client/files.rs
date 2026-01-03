//! Files API client for managing uploaded files.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::form_urlencoded;

use super::messages::ErrorResponse;
use crate::{Error, Result};

const FILES_BASE_URL: &str = "https://api.anthropic.com";
const FILES_API_BETA: &str = "files-api-2025-04-14";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: String,
    #[serde(rename = "type")]
    pub file_type: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub created_at: String,
    #[serde(default)]
    pub downloadable: bool,
}

#[derive(Debug, Clone)]
pub struct UploadFileRequest {
    pub data: FileData,
    pub filename: Option<String>,
}

impl UploadFileRequest {
    pub fn from_bytes(data: Vec<u8>, mime_type: impl Into<String>) -> Self {
        Self {
            data: FileData::Bytes {
                data,
                mime_type: mime_type.into(),
            },
            filename: None,
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            data: FileData::Path(path.into()),
            filename: None,
        }
    }

    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }
}

#[derive(Debug, Clone)]
pub enum FileData {
    Bytes { data: Vec<u8>, mime_type: String },
    Path(PathBuf),
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileListResponse {
    pub data: Vec<File>,
    pub has_more: bool,
    pub first_id: Option<String>,
    pub last_id: Option<String>,
}

pub struct FileDownload {
    response: reqwest::Response,
    pub content_type: String,
    pub content_length: Option<u64>,
}

impl FileDownload {
    pub fn into_response(self) -> reqwest::Response {
        self.response
    }

    pub fn bytes_stream(
        self,
    ) -> impl futures::Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> {
        self.response.bytes_stream()
    }

    pub async fn bytes(self) -> Result<bytes::Bytes> {
        self.response.bytes().await.map_err(Error::Network)
    }
}

pub struct FilesClient<'a> {
    client: &'a super::Client,
}

impl<'a> FilesClient<'a> {
    pub fn new(client: &'a super::Client) -> Self {
        Self { client }
    }

    fn base_url(&self) -> String {
        std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| FILES_BASE_URL.into())
    }

    fn api_version(&self) -> &str {
        &self.client.config().api_version
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}/v1/files{}", self.base_url(), path)
    }

    async fn build_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let req = self.client.http().request(method, url);
        self.client
            .adapter()
            .apply_auth_headers(req)
            .await
            .header("anthropic-version", self.api_version())
            .header("anthropic-beta", FILES_API_BETA)
    }

    async fn send_with_retry(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = req.send().await.map_err(Error::Network)?;

        if response.status().as_u16() == 401 {
            self.client.refresh_credentials().await?;
        }

        Ok(response)
    }

    pub async fn upload(&self, request: UploadFileRequest) -> Result<File> {
        let url = self.build_url("");

        let (data, mime_type, filename) = match request.data {
            FileData::Bytes { data, mime_type } => {
                let filename = request.filename.unwrap_or_else(|| "file".to_string());
                (data, mime_type, filename)
            }
            FileData::Path(path) => {
                let filename = request
                    .filename
                    .or_else(|| path.file_name().and_then(|n| n.to_str()).map(String::from))
                    .unwrap_or_else(|| "file".to_string());

                let data = tokio::fs::read(&path).await.map_err(Error::Io)?;

                let mime_type = mime_guess::from_path(&path)
                    .first_or_octet_stream()
                    .to_string();

                (data, mime_type, filename)
            }
        };

        let part = reqwest::multipart::Part::bytes(data)
            .file_name(filename)
            .mime_str(&mime_type)
            .map_err(|e| Error::Config(e.to_string()))?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let req = self
            .build_request(reqwest::Method::POST, &url)
            .await
            .multipart(form);

        let response = self.send_with_retry(req).await?;
        self.handle_response(response).await
    }

    pub async fn get(&self, file_id: &str) -> Result<File> {
        let url = self.build_url(&format!("/{}", file_id));
        let req = self.build_request(reqwest::Method::GET, &url).await;
        let response = self.send_with_retry(req).await?;
        self.handle_response(response).await
    }

    pub async fn download(&self, file_id: &str) -> Result<FileDownload> {
        let url = self.build_url(&format!("/{}/content", file_id));
        let req = self.build_request(reqwest::Method::GET, &url).await;
        let response = self.send_with_retry(req).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await.map_err(Error::Network)?;
            return Err(error.into_error(status));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let content_length = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());

        Ok(FileDownload {
            response,
            content_type,
            content_length,
        })
    }

    pub async fn download_bytes(&self, file_id: &str) -> Result<Vec<u8>> {
        let download = self.download(file_id).await?;
        let bytes = download.bytes().await?;
        Ok(bytes.to_vec())
    }

    pub async fn delete(&self, file_id: &str) -> Result<()> {
        let url = self.build_url(&format!("/{}", file_id));
        let req = self.build_request(reqwest::Method::DELETE, &url).await;
        let response = self.send_with_retry(req).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await.map_err(Error::Network)?;
            return Err(error.into_error(status));
        }

        Ok(())
    }

    pub async fn list(
        &self,
        limit: Option<u32>,
        after_id: Option<&str>,
    ) -> Result<FileListResponse> {
        let mut url = self.build_url("");

        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = limit {
            query_params.push(("limit", limit.to_string()));
        }
        if let Some(after_id) = after_id {
            query_params.push(("after_id", after_id.to_string()));
        }
        if !query_params.is_empty() {
            let encoded: String = form_urlencoded::Serializer::new(String::new())
                .extend_pairs(query_params.iter().map(|(k, v)| (*k, v.as_str())))
                .finish();
            url = format!("{}?{}", url, encoded);
        }

        let req = self.build_request(reqwest::Method::GET, &url).await;
        let response = self.send_with_retry(req).await?;
        self.handle_response(response).await
    }

    pub async fn list_all(&self) -> Result<Vec<File>> {
        let mut all_files = Vec::new();
        let mut after_id: Option<String> = None;

        loop {
            let response = self.list(Some(100), after_id.as_deref()).await?;
            all_files.extend(response.data);

            if !response.has_more {
                break;
            }
            after_id = response.last_id;
        }

        Ok(all_files)
    }

    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await.map_err(Error::Network)?;
            return Err(error.into_error(status));
        }

        response.json().await.map_err(Error::Network)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_request_from_bytes() {
        let request = UploadFileRequest::from_bytes(vec![1, 2, 3], "image/png");
        assert!(request.filename.is_none());
    }

    #[test]
    fn test_upload_request_with_filename() {
        let request =
            UploadFileRequest::from_bytes(vec![1, 2, 3], "image/png").with_filename("test.png");
        assert_eq!(request.filename, Some("test.png".to_string()));
    }

    #[test]
    fn test_file_deserialization() {
        let json = r#"{
            "id": "file_abc123",
            "type": "file",
            "filename": "test.pdf",
            "mime_type": "application/pdf",
            "size_bytes": 1024,
            "created_at": "2025-01-01T00:00:00Z",
            "downloadable": false
        }"#;
        let file: File = serde_json::from_str(json).unwrap();
        assert_eq!(file.id, "file_abc123");
        assert_eq!(file.filename, "test.pdf");
    }

    #[test]
    fn test_file_list_response_deserialization() {
        let json = r#"{
            "data": [],
            "has_more": false,
            "first_id": null,
            "last_id": null
        }"#;
        let response: FileListResponse = serde_json::from_str(json).unwrap();
        assert!(!response.has_more);
        assert!(response.data.is_empty());
    }
}
