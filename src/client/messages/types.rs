//! Helper types for message requests.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::{ToolDefinition, WebFetchTool, WebSearchTool};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl RequestMetadata {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiTool {
    Custom(ToolDefinition),
    WebSearch(WebSearchTool),
    WebFetch(WebFetchTool),
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

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub error_type: String,
    pub error: ErrorDetail,
}

impl ErrorResponse {
    pub fn into_error(self, status: u16) -> crate::Error {
        crate::Error::Api {
            message: self.error.message,
            status: Some(status),
            error_type: Some(self.error.error_type),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

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
