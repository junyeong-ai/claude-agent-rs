//! Google Vertex AI authentication strategy.

use std::fmt::Debug;

use crate::client::messages::CreateMessageRequest;
use crate::types::SystemPrompt;

use super::env::{env_bool, env_opt, env_with_fallbacks, env_with_fallbacks_or};
use super::traits::AuthStrategy;

/// Google Vertex AI authentication strategy.
///
/// Uses Google Cloud credentials for authentication.
/// Supports both direct GCP credentials and LLM gateway proxies.
#[derive(Clone)]
pub struct VertexStrategy {
    /// GCP project ID
    project_id: String,
    /// Region (e.g., "us-central1", "europe-west1")
    region: String,
    /// Base URL (auto-constructed if not provided)
    base_url: Option<String>,
    /// Skip GCP authentication (for LLM gateways)
    skip_auth: bool,
    /// Access token (if available)
    access_token: Option<String>,
}

impl Debug for VertexStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VertexStrategy")
            .field("project_id", &self.project_id)
            .field("region", &self.region)
            .field("base_url", &self.base_url)
            .field("skip_auth", &self.skip_auth)
            .field("access_token", &self.access_token.as_ref().map(|_| "***"))
            .finish()
    }
}

impl VertexStrategy {
    /// Create a new Vertex AI strategy from environment variables.
    pub fn from_env() -> Option<Self> {
        if !env_bool("CLAUDE_CODE_USE_VERTEX") {
            return None;
        }

        let project_id = env_with_fallbacks(&[
            "ANTHROPIC_VERTEX_PROJECT_ID",
            "GOOGLE_CLOUD_PROJECT",
            "GCLOUD_PROJECT",
        ])?;

        Some(Self {
            project_id,
            region: env_with_fallbacks_or(&["CLOUD_ML_REGION", "GOOGLE_CLOUD_REGION"], "us-central1"),
            base_url: env_opt("ANTHROPIC_VERTEX_BASE_URL"),
            skip_auth: env_bool("CLAUDE_CODE_SKIP_VERTEX_AUTH"),
            access_token: None,
        })
    }

    /// Create with explicit configuration.
    pub fn new(project_id: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            region: region.into(),
            base_url: None,
            skip_auth: false,
            access_token: None,
        }
    }

    /// Set the base URL (for LLM gateways).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Skip GCP authentication (for gateways that handle auth).
    pub fn skip_auth(mut self) -> Self {
        self.skip_auth = true;
        self
    }

    /// Set access token directly.
    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Get the base URL for Vertex AI API.
    pub fn get_base_url(&self) -> String {
        self.base_url.clone().unwrap_or_else(|| {
            format!(
                "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models",
                self.region, self.project_id, self.region
            )
        })
    }

    /// Get the project ID.
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Get the region.
    pub fn region(&self) -> &str {
        &self.region
    }
}

impl AuthStrategy for VertexStrategy {
    fn auth_header(&self) -> (&'static str, String) {
        if let Some(ref token) = self.access_token {
            ("Authorization", format!("Bearer {}", token))
        } else {
            // OAuth token will be obtained separately
            ("Authorization", "Bearer <pending>".to_string())
        }
    }

    fn extra_headers(&self) -> Vec<(String, String)> {
        vec![
            ("x-goog-user-project".to_string(), self.project_id.clone()),
        ]
    }

    fn url_query_string(&self) -> Option<String> {
        None
    }

    fn prepare_system_prompt(
        &self,
        existing: Option<SystemPrompt>,
    ) -> Option<SystemPrompt> {
        // Vertex AI doesn't require special system prompt handling
        existing
    }

    fn prepare_metadata(&self) -> Option<crate::client::messages::RequestMetadata> {
        None
    }

    fn prepare_request(&self, request: CreateMessageRequest) -> CreateMessageRequest {
        request
    }

    fn name(&self) -> &'static str {
        "vertex"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_strategy_creation() {
        let strategy = VertexStrategy::new("my-project", "us-central1");
        assert_eq!(strategy.project_id(), "my-project");
        assert_eq!(strategy.region(), "us-central1");
        assert_eq!(strategy.name(), "vertex");
    }

    #[test]
    fn test_vertex_base_url() {
        let strategy = VertexStrategy::new("my-project", "us-central1");
        let url = strategy.get_base_url();
        assert!(url.contains("my-project"));
        assert!(url.contains("us-central1"));

        let custom = VertexStrategy::new("my-project", "us-central1")
            .with_base_url("https://my-gateway.com/vertex");
        assert_eq!(custom.get_base_url(), "https://my-gateway.com/vertex");
    }

    #[test]
    fn test_vertex_skip_auth() {
        let strategy = VertexStrategy::new("p", "r").skip_auth();
        assert!(strategy.skip_auth);
    }

    #[test]
    fn test_vertex_extra_headers() {
        let strategy = VertexStrategy::new("my-project", "us-central1");
        let headers = strategy.extra_headers();
        assert!(headers.iter().any(|(k, v)| k == "x-goog-user-project" && v == "my-project"));
    }
}
