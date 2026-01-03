//! Batch Processing API for large-scale asynchronous requests.

use serde::{Deserialize, Serialize};
use url::form_urlencoded;

use super::messages::{CreateMessageRequest, ErrorResponse};
use crate::types::ApiResponse;

const BATCH_BASE_URL: &str = "https://api.anthropic.com";

#[derive(Debug, Clone, Serialize)]
pub struct BatchRequest {
    pub custom_id: String,
    pub params: CreateMessageRequest,
}

impl BatchRequest {
    pub fn new(custom_id: impl Into<String>, params: CreateMessageRequest) -> Self {
        Self {
            custom_id: custom_id.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateBatchRequest {
    pub requests: Vec<BatchRequest>,
}

impl CreateBatchRequest {
    pub fn new(requests: Vec<BatchRequest>) -> Self {
        Self { requests }
    }

    pub fn with_request(mut self, request: BatchRequest) -> Self {
        self.requests.push(request);
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageBatch {
    pub id: String,
    #[serde(rename = "type")]
    pub batch_type: String,
    pub processing_status: BatchStatus,
    pub request_counts: RequestCounts,
    pub ended_at: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub cancel_initiated_at: Option<String>,
    pub results_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    InProgress,
    Canceling,
    Ended,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct RequestCounts {
    pub processing: u32,
    pub succeeded: u32,
    pub errored: u32,
    pub canceled: u32,
    pub expired: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchResult {
    pub custom_id: String,
    pub result: BatchResultType,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchResultType {
    Succeeded { message: ApiResponse },
    Errored { error: BatchError },
    Canceled,
    Expired,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchListResponse {
    pub data: Vec<MessageBatch>,
    pub has_more: bool,
    pub first_id: Option<String>,
    pub last_id: Option<String>,
}

pub struct BatchClient<'a> {
    client: &'a super::Client,
}

impl<'a> BatchClient<'a> {
    pub fn new(client: &'a super::Client) -> Self {
        Self { client }
    }

    fn base_url(&self) -> String {
        std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| BATCH_BASE_URL.into())
    }

    fn api_version(&self) -> &str {
        &self.client.config().api_version
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}/v1/messages/batches{}", self.base_url(), path)
    }

    async fn build_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .http()
            .request(method, url)
            .header("anthropic-version", self.api_version())
            .header("content-type", "application/json");

        request = self.client.adapter().apply_auth_headers(request).await;

        if let Some(beta_header) = self.client.config().beta.header_value() {
            request = request.header("anthropic-beta", beta_header);
        }

        request
    }

    async fn send_with_retry(
        &self,
        request: reqwest::RequestBuilder,
    ) -> crate::Result<reqwest::Response> {
        let response = request.send().await?;

        if response.status().as_u16() == 401 {
            self.client.refresh_credentials().await?;
        }

        Ok(response)
    }

    pub async fn create(&self, request: CreateBatchRequest) -> crate::Result<MessageBatch> {
        let url = self.build_url("");
        let request = self
            .build_request(reqwest::Method::POST, &url)
            .await
            .json(&request);
        let response = self.send_with_retry(request).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await?;
            return Err(error.into_error(status));
        }

        Ok(response.json().await?)
    }

    pub async fn get(&self, batch_id: &str) -> crate::Result<MessageBatch> {
        let url = self.build_url(&format!("/{}", batch_id));
        let request = self.build_request(reqwest::Method::GET, &url).await;
        let response = self.send_with_retry(request).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await?;
            return Err(error.into_error(status));
        }

        Ok(response.json().await?)
    }

    pub async fn cancel(&self, batch_id: &str) -> crate::Result<MessageBatch> {
        let url = self.build_url(&format!("/{}/cancel", batch_id));
        let request = self.build_request(reqwest::Method::POST, &url).await;
        let response = self.send_with_retry(request).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await?;
            return Err(error.into_error(status));
        }

        Ok(response.json().await?)
    }

    pub async fn list(
        &self,
        limit: Option<u32>,
        after_id: Option<&str>,
    ) -> crate::Result<BatchListResponse> {
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

        let request = self.build_request(reqwest::Method::GET, &url).await;
        let response = self.send_with_retry(request).await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await?;
            return Err(error.into_error(status));
        }

        Ok(response.json().await?)
    }

    pub async fn results(&self, batch_id: &str) -> crate::Result<Vec<BatchResult>> {
        let batch = self.get(batch_id).await?;

        let results_url = batch.results_url.ok_or_else(|| crate::Error::Api {
            message: "Batch results not yet available".to_string(),
            status: None,
            error_type: None,
        })?;

        let mut request = self
            .client
            .http()
            .get(&results_url)
            .header("anthropic-version", self.api_version());

        request = self.client.adapter().apply_auth_headers(request).await;

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(crate::Error::Api {
                message: format!("Failed to fetch batch results: HTTP {}", status),
                status: Some(status),
                error_type: None,
            });
        }

        let text = response.text().await?;
        let results: Vec<BatchResult> = text
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        Ok(results)
    }

    pub async fn wait_for_completion(
        &self,
        batch_id: &str,
        poll_interval: std::time::Duration,
    ) -> crate::Result<MessageBatch> {
        loop {
            let batch = self.get(batch_id).await?;
            if batch.processing_status == BatchStatus::Ended {
                return Ok(batch);
            }
            tokio::time::sleep(poll_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_request_serialization() {
        let request = BatchRequest::new(
            "test-1",
            CreateMessageRequest::new(
                "claude-sonnet-4-5",
                vec![crate::types::Message::user("Hello")],
            ),
        );
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test-1"));
    }

    #[test]
    fn test_batch_status_deserialization() {
        let json = r#""in_progress""#;
        let status: BatchStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status, BatchStatus::InProgress);
    }

    #[test]
    fn test_batch_status_all_variants() {
        assert_eq!(
            serde_json::from_str::<BatchStatus>(r#""canceling""#).unwrap(),
            BatchStatus::Canceling
        );
        assert_eq!(
            serde_json::from_str::<BatchStatus>(r#""ended""#).unwrap(),
            BatchStatus::Ended
        );
    }

    #[test]
    fn test_create_batch_request_builder() {
        let req1 = BatchRequest::new(
            "req-1",
            CreateMessageRequest::new("claude-sonnet-4-5", vec![crate::types::Message::user("A")]),
        );
        let req2 = BatchRequest::new(
            "req-2",
            CreateMessageRequest::new("claude-sonnet-4-5", vec![crate::types::Message::user("B")]),
        );

        let batch = CreateBatchRequest::new(vec![req1]).with_request(req2);
        assert_eq!(batch.requests.len(), 2);
        assert_eq!(batch.requests[0].custom_id, "req-1");
        assert_eq!(batch.requests[1].custom_id, "req-2");
    }

    #[test]
    fn test_request_counts_default() {
        let counts = RequestCounts::default();
        assert_eq!(counts.processing, 0);
        assert_eq!(counts.succeeded, 0);
        assert_eq!(counts.errored, 0);
        assert_eq!(counts.canceled, 0);
        assert_eq!(counts.expired, 0);
    }

    #[test]
    fn test_request_counts_deserialization() {
        let json = r#"{"processing":5,"succeeded":10,"errored":2,"canceled":1,"expired":0}"#;
        let counts: RequestCounts = serde_json::from_str(json).unwrap();
        assert_eq!(counts.processing, 5);
        assert_eq!(counts.succeeded, 10);
        assert_eq!(counts.errored, 2);
        assert_eq!(counts.canceled, 1);
        assert_eq!(counts.expired, 0);
    }

    #[test]
    fn test_batch_error_deserialization() {
        let json = r#"{"type":"invalid_request","message":"Bad input"}"#;
        let error: BatchError = serde_json::from_str(json).unwrap();
        assert_eq!(error.error_type, "invalid_request");
        assert_eq!(error.message, "Bad input");
    }

    #[test]
    fn test_batch_result_succeeded() {
        let json = r#"{
            "custom_id": "req-1",
            "result": {
                "type": "succeeded",
                "message": {
                    "id": "msg_123",
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "text", "text": "Hello"}],
                    "model": "claude-sonnet-4-5",
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 10, "output_tokens": 5}
                }
            }
        }"#;
        let result: BatchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.custom_id, "req-1");
        assert!(matches!(result.result, BatchResultType::Succeeded { .. }));
    }

    #[test]
    fn test_batch_result_errored() {
        let json = r#"{
            "custom_id": "req-2",
            "result": {
                "type": "errored",
                "error": {
                    "type": "rate_limit",
                    "message": "Too many requests"
                }
            }
        }"#;
        let result: BatchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.custom_id, "req-2");
        if let BatchResultType::Errored { error } = result.result {
            assert_eq!(error.error_type, "rate_limit");
            assert_eq!(error.message, "Too many requests");
        } else {
            panic!("Expected Errored variant");
        }
    }

    #[test]
    fn test_batch_result_canceled() {
        let json = r#"{"custom_id": "req-3", "result": {"type": "canceled"}}"#;
        let result: BatchResult = serde_json::from_str(json).unwrap();
        assert!(matches!(result.result, BatchResultType::Canceled));
    }

    #[test]
    fn test_batch_result_expired() {
        let json = r#"{"custom_id": "req-4", "result": {"type": "expired"}}"#;
        let result: BatchResult = serde_json::from_str(json).unwrap();
        assert!(matches!(result.result, BatchResultType::Expired));
    }

    #[test]
    fn test_message_batch_deserialization() {
        let json = r#"{
            "id": "batch_123",
            "type": "message_batch",
            "processing_status": "in_progress",
            "request_counts": {"processing": 5, "succeeded": 0, "errored": 0, "canceled": 0, "expired": 0},
            "created_at": "2024-01-01T00:00:00Z",
            "expires_at": "2024-01-02T00:00:00Z",
            "ended_at": null,
            "cancel_initiated_at": null,
            "results_url": null
        }"#;
        let batch: MessageBatch = serde_json::from_str(json).unwrap();
        assert_eq!(batch.id, "batch_123");
        assert_eq!(batch.processing_status, BatchStatus::InProgress);
        assert_eq!(batch.request_counts.processing, 5);
        assert!(batch.ended_at.is_none());
        assert!(batch.results_url.is_none());
    }

    #[test]
    fn test_batch_list_response() {
        let json = r#"{
            "data": [],
            "has_more": false,
            "first_id": null,
            "last_id": null
        }"#;
        let response: BatchListResponse = serde_json::from_str(json).unwrap();
        assert!(response.data.is_empty());
        assert!(!response.has_more);
    }

    #[test]
    fn test_batch_request_with_all_params() {
        let request = CreateMessageRequest::new(
            "claude-sonnet-4-5",
            vec![crate::types::Message::user("Test")],
        )
        .with_max_tokens(1000)
        .with_temperature(0.5);

        let batch_req = BatchRequest::new("custom-id-123", request);
        assert_eq!(batch_req.custom_id, "custom-id-123");
        assert_eq!(batch_req.params.max_tokens, 1000);
    }
}
