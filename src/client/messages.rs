//! Messages API implementation.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::{ApiResponse, Message, SystemPrompt, ToolDefinition, WebSearchTool};

/// Metadata for the Messages API request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestMetadata {
    /// User ID for tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Additional metadata fields.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl RequestMetadata {
    /// Generate metadata for OAuth authentication.
    pub fn generate() -> Self {
        let session_id = uuid::Uuid::new_v4();
        let user_hash = format!("{:x}", simple_hash(session_id.as_bytes()));
        let account_uuid = uuid::Uuid::new_v4();

        Self {
            user_id: Some(format!(
                "user_{}_account_{}_session_{}",
                user_hash, account_uuid, session_id
            )),
            extra: HashMap::new(),
        }
    }
}

fn simple_hash(data: &[u8]) -> u128 {
    let mut hash: u128 = 0;
    for (i, &byte) in data.iter().enumerate() {
        hash = hash.wrapping_add((byte as u128).wrapping_mul((i as u128).wrapping_add(1)));
        hash = hash.wrapping_mul(31);
    }
    hash
}

/// A tool that can be provided to the API (custom or built-in).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiTool {
    /// Custom tool with schema.
    Custom(ToolDefinition),
    /// Built-in web search.
    WebSearch(WebSearchTool),
}

impl From<ToolDefinition> for ApiTool {
    fn from(tool: ToolDefinition) -> Self {
        Self::Custom(tool)
    }
}

impl From<WebSearchTool> for ApiTool {
    fn from(tool: WebSearchTool) -> Self {
        Self::WebSearch(tool)
    }
}

/// Request body for the Messages API.
#[derive(Debug, Clone, Serialize)]
pub struct CreateMessageRequest {
    /// Model identifier.
    pub model: String,
    /// Maximum tokens to generate.
    pub max_tokens: u32,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    /// Available tools (custom and built-in).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ApiTool>>,
    /// Enable streaming.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Temperature (0.0-1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-p sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Top-k sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RequestMetadata>,
}

impl CreateMessageRequest {
    /// Create a new request.
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            max_tokens: 8192,
            messages,
            system: None,
            tools: None,
            stream: None,
            stop_sequences: None,
            temperature: None,
            top_p: None,
            top_k: None,
            metadata: None,
        }
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: RequestMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Set system prompt.
    pub fn with_system(mut self, system: impl Into<SystemPrompt>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Set available tools.
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        let api_tools: Vec<ApiTool> = tools.into_iter().map(ApiTool::Custom).collect();
        self.tools = Some(api_tools);
        self
    }

    /// Add web search capability (Anthropic built-in).
    pub fn with_web_search(mut self, config: WebSearchTool) -> Self {
        let mut tools = self.tools.unwrap_or_default();
        tools.push(ApiTool::WebSearch(config));
        self.tools = Some(tools);
        self
    }

    /// Set all tools (custom and built-in).
    pub fn with_api_tools(mut self, tools: Vec<ApiTool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Enable streaming.
    pub fn with_stream(mut self) -> Self {
        self.stream = Some(true);
        self
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

impl From<String> for SystemPrompt {
    fn from(s: String) -> Self {
        SystemPrompt::Text(s)
    }
}

impl From<&str> for SystemPrompt {
    fn from(s: &str) -> Self {
        SystemPrompt::Text(s.to_string())
    }
}

/// Error response from the API.
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorResponse {
    /// Error type.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error details.
    pub error: ErrorDetail,
}

/// Error detail in API response.
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorDetail {
    /// Error type.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error message.
    pub message: String,
}

/// Client for the Messages API.
pub struct MessagesClient<'a> {
    client: &'a super::Client,
}

impl<'a> MessagesClient<'a> {
    /// Create a new Messages client.
    pub fn new(client: &'a super::Client) -> Self {
        Self { client }
    }

    /// Build a configured HTTP request.
    fn build_request(&self, request: CreateMessageRequest) -> (reqwest::RequestBuilder, String) {
        let strategy = &self.client.config().auth_strategy;
        let base_url = &self.client.config().base_url;

        let url = match strategy.url_query_string() {
            Some(query) => format!("{}/v1/messages?{}", base_url, query),
            None => format!("{}/v1/messages", base_url),
        };

        let request = strategy.prepare_request(request);
        let (header_name, header_value) = strategy.auth_header();

        let mut req = self
            .client
            .http()
            .post(&url)
            .header(header_name, header_value)
            .header("anthropic-version", &self.client.config().api_version)
            .header("content-type", "application/json");

        for (name, value) in strategy.extra_headers() {
            req = req.header(name, value);
        }

        (req.json(&request), url)
    }

    /// Create a message (non-streaming).
    pub async fn create(&self, request: CreateMessageRequest) -> crate::Result<ApiResponse> {
        let (req, _url) = self.build_request(request);
        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await?;
            return Err(crate::Error::Api {
                message: error.error.message,
                status: Some(status),
            });
        }

        let api_response: ApiResponse = response.json().await?;
        Ok(api_response)
    }

    /// Create a message with streaming.
    pub async fn create_stream(
        &self,
        request: CreateMessageRequest,
    ) -> crate::Result<
        impl futures::Stream<Item = crate::Result<super::StreamItem>> + Send + 'static + use<>,
    > {
        let mut request = request;
        request.stream = Some(true);

        let (req, _url) = self.build_request(request);
        let response = req.send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error: ErrorResponse = response.json().await?;
            return Err(crate::Error::Api {
                message: error.error.message,
                status: Some(status),
            });
        }

        Ok(super::StreamParser::new(response.bytes_stream()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_request() {
        let request = CreateMessageRequest::new("claude-sonnet-4-5", vec![Message::user("Hello")])
            .with_max_tokens(1000)
            .with_temperature(0.7);

        assert_eq!(request.model, "claude-sonnet-4-5");
        assert_eq!(request.max_tokens, 1000);
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_request_metadata_generate() {
        let metadata = RequestMetadata::generate();
        assert!(metadata.user_id.is_some());
        let user_id = metadata.user_id.unwrap();
        assert!(user_id.starts_with("user_"));
        assert!(user_id.contains("_account_"));
        assert!(user_id.contains("_session_"));
    }
}
