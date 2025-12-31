//! OAuth authentication strategy for Claude Code CLI tokens.

use super::AuthStrategy;
use crate::auth::config::OAuthConfig;
use crate::auth::OAuthCredential;
use crate::client::messages::RequestMetadata;
use crate::types::{CacheControl, SystemBlock, SystemPrompt};

/// OAuth authentication strategy.
/// Uses Claude Code CLI tokens with configurable headers and system prompts.
#[derive(Debug, Clone)]
pub struct OAuthStrategy {
    credential: OAuthCredential,
    config: OAuthConfig,
}

impl OAuthStrategy {
    /// Create with default configuration (environment variables applied).
    pub fn new(credential: OAuthCredential) -> Self {
        Self {
            credential,
            config: OAuthConfig::from_env(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(credential: OAuthCredential, config: OAuthConfig) -> Self {
        Self { credential, config }
    }

    /// Access the configuration.
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }

    /// Mutably access the configuration.
    pub fn config_mut(&mut self) -> &mut OAuthConfig {
        &mut self.config
    }

    /// Access the credential.
    pub fn credential(&self) -> &OAuthCredential {
        &self.credential
    }
}

impl AuthStrategy for OAuthStrategy {
    fn auth_header(&self) -> (&'static str, String) {
        (
            "Authorization",
            format!("Bearer {}", self.credential.access_token),
        )
    }

    fn extra_headers(&self) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        if !self.config.beta_flags.is_empty() {
            headers.push((
                "anthropic-beta".to_string(),
                self.config.beta_header_value(),
            ));
        }

        headers.push(("user-agent".to_string(), self.config.user_agent.clone()));
        headers.push(("x-app".to_string(), self.config.app_identifier.clone()));

        for (k, v) in &self.config.extra_headers {
            headers.push((k.clone(), v.clone()));
        }

        headers
    }

    fn url_query_string(&self) -> Option<String> {
        if self.config.url_params.is_empty() {
            return None;
        }

        let params: Vec<String> = self
            .config
            .url_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        Some(params.join("&"))
    }

    fn prepare_system_prompt(&self, existing: Option<SystemPrompt>) -> Option<SystemPrompt> {
        let claude_code_block = SystemBlock {
            block_type: "text".to_string(),
            text: self.config.system_prompt.clone(),
            cache_control: Some(CacheControl {
                cache_type: "ephemeral".to_string(),
            }),
        };

        Some(match existing {
            None => SystemPrompt::Blocks(vec![claude_code_block]),
            Some(SystemPrompt::Text(text)) => {
                let user_block = SystemBlock {
                    block_type: "text".to_string(),
                    text,
                    cache_control: None,
                };
                SystemPrompt::Blocks(vec![claude_code_block, user_block])
            }
            Some(SystemPrompt::Blocks(mut blocks)) => {
                blocks.insert(0, claude_code_block);
                SystemPrompt::Blocks(blocks)
            }
        })
    }

    fn prepare_metadata(&self) -> Option<RequestMetadata> {
        Some(RequestMetadata::generate())
    }

    fn name(&self) -> &'static str {
        "oauth"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_credential() -> OAuthCredential {
        OAuthCredential {
            access_token: "sk-ant-oat01-test".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        }
    }

    #[test]
    fn test_auth_header() {
        let strategy = OAuthStrategy::new(test_credential());
        let (name, value) = strategy.auth_header();
        assert_eq!(name, "Authorization");
        assert_eq!(value, "Bearer sk-ant-oat01-test");
    }

    #[test]
    fn test_extra_headers() {
        let strategy = OAuthStrategy::new(test_credential());
        let headers = strategy.extra_headers();

        let header_map: std::collections::HashMap<_, _> = headers.into_iter().collect();

        assert!(header_map.contains_key("anthropic-beta"));
        assert!(header_map.contains_key("user-agent"));
        assert!(header_map.contains_key("x-app"));
        assert!(header_map.contains_key("anthropic-dangerous-direct-browser-access"));
    }

    #[test]
    fn test_url_query_string() {
        let strategy = OAuthStrategy::new(test_credential());
        let query = strategy.url_query_string();
        assert!(query.is_some());
        assert!(query.unwrap().contains("beta=true"));
    }

    #[test]
    fn test_system_prompt_prepend() {
        let strategy = OAuthStrategy::new(test_credential());

        // Test with no existing prompt
        let result = strategy.prepare_system_prompt(None);
        assert!(result.is_some());

        // Test with existing text prompt
        let existing = SystemPrompt::text("User prompt");
        let result = strategy.prepare_system_prompt(Some(existing));
        if let Some(SystemPrompt::Blocks(blocks)) = result {
            assert_eq!(blocks.len(), 2);
            assert!(blocks[0].text.contains("Claude Code"));
        } else {
            panic!("Expected Blocks variant");
        }
    }

    #[test]
    fn test_custom_config() {
        let config = OAuthConfig::builder()
            .system_prompt("Custom prompt")
            .user_agent("custom-agent/1.0")
            .build();

        let strategy = OAuthStrategy::with_config(test_credential(), config);
        assert_eq!(strategy.config().system_prompt, "Custom prompt");
        assert_eq!(strategy.config().user_agent, "custom-agent/1.0");
    }
}
