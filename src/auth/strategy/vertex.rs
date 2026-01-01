//! Google Vertex AI authentication strategy.

use std::collections::HashMap;
use std::fmt::Debug;

use super::env::{env_bool, env_opt, env_with_fallbacks, env_with_fallbacks_or};
use super::traits::AuthStrategy;

/// Google Vertex AI authentication strategy.
///
/// Supports:
/// - Google Cloud credentials (access token)
/// - Global and regional endpoints
/// - Per-model region overrides
/// - LLM gateway passthrough
#[derive(Clone)]
pub struct VertexStrategy {
    project_id: String,
    region: String,
    base_url: Option<String>,
    skip_auth: bool,
    access_token: Option<String>,
    model_region_overrides: HashMap<String, String>,
    disable_caching: bool,
    use_1m_context: bool,
}

impl Debug for VertexStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VertexStrategy")
            .field("project_id", &self.project_id)
            .field("region", &self.region)
            .field("base_url", &self.base_url)
            .field("skip_auth", &self.skip_auth)
            .field("has_access_token", &self.access_token.is_some())
            .field("model_region_overrides", &self.model_region_overrides)
            .field("use_1m_context", &self.use_1m_context)
            .finish()
    }
}

impl VertexStrategy {
    /// Create from environment variables.
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
            region: env_with_fallbacks_or(
                &["CLOUD_ML_REGION", "GOOGLE_CLOUD_REGION"],
                "us-central1",
            ),
            base_url: env_opt("ANTHROPIC_VERTEX_BASE_URL"),
            skip_auth: env_bool("CLAUDE_CODE_SKIP_VERTEX_AUTH"),
            access_token: None,
            model_region_overrides: Self::load_model_region_overrides(),
            disable_caching: env_bool("DISABLE_PROMPT_CACHING"),
            use_1m_context: env_bool("VERTEX_USE_1M_CONTEXT"),
        })
    }

    /// Load per-model region overrides from environment.
    fn load_model_region_overrides() -> HashMap<String, String> {
        let mut overrides = HashMap::new();

        let model_vars = [
            ("VERTEX_REGION_CLAUDE_3_5_HAIKU", "claude-3-5-haiku"),
            ("VERTEX_REGION_CLAUDE_3_5_SONNET", "claude-3-5-sonnet"),
            ("VERTEX_REGION_CLAUDE_3_7_SONNET", "claude-3-7-sonnet"),
            ("VERTEX_REGION_CLAUDE_4_0_OPUS", "claude-4-0-opus"),
            ("VERTEX_REGION_CLAUDE_4_0_SONNET", "claude-4-0-sonnet"),
            ("VERTEX_REGION_CLAUDE_4_5_SONNET", "claude-4-5-sonnet"),
            ("VERTEX_REGION_CLAUDE_4_5_HAIKU", "claude-4-5-haiku"),
            ("VERTEX_REGION_CLAUDE_OPUS_4_1", "claude-opus-4-1"),
        ];

        for (env_var, model_key) in model_vars {
            if let Ok(region) = std::env::var(env_var) {
                overrides.insert(model_key.to_string(), region);
            }
        }

        overrides
    }

    /// Create with explicit configuration.
    pub fn new(project_id: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            region: region.into(),
            base_url: None,
            skip_auth: false,
            access_token: None,
            model_region_overrides: HashMap::new(),
            disable_caching: false,
            use_1m_context: false,
        }
    }

    /// Set base URL for LLM gateway.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Skip GCP authentication (for gateways).
    pub fn skip_auth(mut self) -> Self {
        self.skip_auth = true;
        self
    }

    /// Set access token directly.
    pub fn with_access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Add a model-specific region override.
    pub fn with_model_region(
        mut self,
        model: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        self.model_region_overrides
            .insert(model.into(), region.into());
        self
    }

    /// Disable prompt caching.
    pub fn disable_caching(mut self) -> Self {
        self.disable_caching = true;
        self
    }

    /// Enable 1M token context (beta feature).
    pub fn enable_1m_context(mut self) -> Self {
        self.use_1m_context = true;
        self
    }

    /// Check if using global endpoint.
    pub fn is_global(&self) -> bool {
        self.region == "global"
    }

    /// Get the effective region for a specific model.
    pub fn get_region_for_model(&self, model: &str) -> &str {
        // Extract base model name (e.g., "claude-sonnet-4-5@20250929" -> "claude-sonnet-4-5")
        let base_name = model.split('@').next().unwrap_or(model);

        // Normalize to lookup format
        let normalized = base_name.replace('_', "-");

        // Check for exact match or partial match
        for (key, region) in &self.model_region_overrides {
            if normalized.contains(key) {
                return region;
            }
        }

        &self.region
    }

    /// Get base URL for Vertex AI API.
    pub fn get_base_url(&self) -> String {
        self.base_url.clone().unwrap_or_else(|| {
            format!(
                "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models",
                self.region, self.project_id, self.region
            )
        })
    }

    /// Get base URL for a specific model (respects region overrides).
    pub fn get_base_url_for_model(&self, model: &str) -> String {
        if self.base_url.is_some() {
            return self.get_base_url();
        }

        let region = self.get_region_for_model(model);
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models",
            region, self.project_id, region
        )
    }

    /// Get the project ID.
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    /// Get the default region.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Check if prompt caching is disabled.
    pub fn is_caching_disabled(&self) -> bool {
        self.disable_caching
    }
}

impl AuthStrategy for VertexStrategy {
    fn auth_header(&self) -> (&'static str, String) {
        if let Some(ref token) = self.access_token {
            ("Authorization", format!("Bearer {}", token))
        } else {
            ("Authorization", "Bearer <pending>".to_string())
        }
    }

    fn extra_headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![("x-goog-user-project".to_string(), self.project_id.clone())];

        if self.use_1m_context {
            headers.push((
                "anthropic-beta".to_string(),
                "context-1m-2025-08-07".to_string(),
            ));
        }

        headers
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
    }

    #[test]
    fn test_vertex_global_region() {
        let strategy = VertexStrategy::new("my-project", "global");
        assert!(strategy.is_global());
    }

    #[test]
    fn test_vertex_model_region_override() {
        let strategy = VertexStrategy::new("my-project", "us-central1")
            .with_model_region("claude-3-5-haiku", "us-east5");

        assert_eq!(
            strategy.get_region_for_model("claude-3-5-haiku@20241022"),
            "us-east5"
        );
        assert_eq!(
            strategy.get_region_for_model("claude-sonnet-4-5@20250929"),
            "us-central1"
        );
    }

    #[test]
    fn test_vertex_1m_context() {
        let strategy = VertexStrategy::new("p", "r").enable_1m_context();
        let headers = strategy.extra_headers();
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "anthropic-beta" && v.contains("context-1m"))
        );
    }

    #[test]
    fn test_vertex_skip_auth() {
        let strategy = VertexStrategy::new("p", "r").skip_auth();
        assert!(strategy.skip_auth);
    }
}
