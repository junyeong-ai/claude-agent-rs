//! API Key authentication strategy.

use super::AuthStrategy;
use crate::client::messages::RequestMetadata;
use crate::types::SystemPrompt;

/// API Key authentication strategy.
/// Simple authentication with no additional headers or request modifications.
#[derive(Debug, Clone)]
pub struct ApiKeyStrategy {
    key: String,
}

impl ApiKeyStrategy {
    /// Create a new API Key strategy.
    pub fn new(key: impl Into<String>) -> Self {
        Self { key: key.into() }
    }
}

impl AuthStrategy for ApiKeyStrategy {
    fn auth_header(&self) -> (&'static str, String) {
        ("x-api-key", self.key.clone())
    }

    fn extra_headers(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn url_query_string(&self) -> Option<String> {
        None
    }

    fn prepare_system_prompt(&self, existing: Option<SystemPrompt>) -> Option<SystemPrompt> {
        existing
    }

    fn prepare_metadata(&self) -> Option<RequestMetadata> {
        None
    }

    fn name(&self) -> &'static str {
        "api_key"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_header() {
        let strategy = ApiKeyStrategy::new("sk-ant-api-test");
        let (name, value) = strategy.auth_header();
        assert_eq!(name, "x-api-key");
        assert_eq!(value, "sk-ant-api-test");
    }

    #[test]
    fn test_no_extra_headers() {
        let strategy = ApiKeyStrategy::new("test");
        assert!(strategy.extra_headers().is_empty());
    }

    #[test]
    fn test_no_url_params() {
        let strategy = ApiKeyStrategy::new("test");
        assert!(strategy.url_query_string().is_none());
    }

    #[test]
    fn test_system_prompt_passthrough() {
        let strategy = ApiKeyStrategy::new("test");
        let prompt = SystemPrompt::text("Hello");
        let result = strategy.prepare_system_prompt(Some(prompt.clone()));
        assert!(result.is_some());
    }
}
