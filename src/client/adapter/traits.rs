//! Provider adapter trait definition.

use std::fmt::Debug;

use async_trait::async_trait;

use super::config::{ModelType, ProviderConfig};
use crate::client::messages::{CountTokensRequest, CountTokensResponse, CreateMessageRequest};
use crate::types::ApiResponse;
use crate::{Error, Result};

#[async_trait]
pub trait ProviderAdapter: Send + Sync + Debug {
    fn config(&self) -> &ProviderConfig;

    fn name(&self) -> &'static str;

    fn model(&self, model_type: ModelType) -> &str {
        self.config().models.get(model_type)
    }

    async fn build_url(&self, model: &str, stream: bool) -> String;

    async fn prepare_request(&self, request: CreateMessageRequest) -> CreateMessageRequest {
        request
    }

    async fn transform_request(&self, request: CreateMessageRequest) -> Result<serde_json::Value>;

    fn transform_response(&self, response: serde_json::Value) -> Result<ApiResponse>;

    async fn apply_auth_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req
    }

    async fn send(
        &self,
        http: &reqwest::Client,
        request: CreateMessageRequest,
    ) -> Result<ApiResponse>;

    async fn send_stream(
        &self,
        http: &reqwest::Client,
        request: CreateMessageRequest,
    ) -> Result<reqwest::Response>;

    async fn refresh_credentials(&self) -> Result<()> {
        Ok(())
    }

    async fn count_tokens(
        &self,
        _http: &reqwest::Client,
        _request: CountTokensRequest,
    ) -> Result<CountTokensResponse> {
        Err(Error::NotSupported {
            provider: self.name(),
            operation: "count_tokens",
        })
    }
}
