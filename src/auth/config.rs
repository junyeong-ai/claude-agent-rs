//! OAuth configuration for Claude Code CLI authentication.

use std::collections::HashMap;

/// Default system prompt required for OAuth authentication.
pub const DEFAULT_SYSTEM_PROMPT: &str =
    "You are Claude Code, Anthropic's official CLI for Claude.";

/// Default beta flags for Claude Code CLI.
pub const DEFAULT_BETA_FLAGS: &[&str] = &[
    "claude-code-20250219",
    "oauth-2025-04-20",
    "interleaved-thinking-2025-05-14",
];

/// Default user agent for Claude Code CLI.
pub const DEFAULT_USER_AGENT: &str = "claude-cli/2.0.76 (external, cli)";

/// Default app identifier.
pub const DEFAULT_APP_IDENTIFIER: &str = "cli";

/// Configuration for OAuth authentication.
/// All fields have sensible defaults that can be overridden via environment variables or code.
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// System prompt prepended to all requests (required for OAuth).
    pub system_prompt: String,
    /// Beta flags for the anthropic-beta header.
    pub beta_flags: Vec<String>,
    /// User-Agent header value.
    pub user_agent: String,
    /// x-app header value.
    pub app_identifier: String,
    /// URL query parameters (e.g., beta=true).
    pub url_params: HashMap<String, String>,
    /// Additional headers.
    pub extra_headers: HashMap<String, String>,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            beta_flags: DEFAULT_BETA_FLAGS.iter().map(|s| s.to_string()).collect(),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            app_identifier: DEFAULT_APP_IDENTIFIER.to_string(),
            url_params: [("beta".to_string(), "true".to_string())]
                .into_iter()
                .collect(),
            extra_headers: [(
                "anthropic-dangerous-direct-browser-access".to_string(),
                "true".to_string(),
            )]
            .into_iter()
            .collect(),
        }
    }
}

impl OAuthConfig {
    /// Create configuration with defaults, then apply environment variable overrides.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(prompt) = std::env::var("CLAUDE_AGENT_SYSTEM_PROMPT") {
            config.system_prompt = prompt;
        }
        if let Ok(flags) = std::env::var("CLAUDE_AGENT_BETA_FLAGS") {
            config.beta_flags = flags.split(',').map(|s| s.trim().to_string()).collect();
        }
        if let Ok(ua) = std::env::var("CLAUDE_AGENT_USER_AGENT") {
            config.user_agent = ua;
        }
        if let Ok(app) = std::env::var("CLAUDE_AGENT_APP_IDENTIFIER") {
            config.app_identifier = app;
        }

        config
    }

    /// Create a builder for custom configuration.
    pub fn builder() -> OAuthConfigBuilder {
        OAuthConfigBuilder::default()
    }

    /// Get the anthropic-beta header value.
    pub fn beta_header_value(&self) -> String {
        self.beta_flags.join(",")
    }
}

/// Builder for OAuthConfig.
pub struct OAuthConfigBuilder {
    config: OAuthConfig,
}

impl Default for OAuthConfigBuilder {
    fn default() -> Self {
        Self {
            config: OAuthConfig::from_env(),
        }
    }
}

impl OAuthConfigBuilder {
    /// Set the system prompt.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.system_prompt = prompt.into();
        self
    }

    /// Set beta flags (replaces all existing flags).
    pub fn beta_flags(mut self, flags: Vec<String>) -> Self {
        self.config.beta_flags = flags;
        self
    }

    /// Add a single beta flag.
    pub fn add_beta_flag(mut self, flag: impl Into<String>) -> Self {
        self.config.beta_flags.push(flag.into());
        self
    }

    /// Set the user agent.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.config.user_agent = ua.into();
        self
    }

    /// Set the app identifier.
    pub fn app_identifier(mut self, app: impl Into<String>) -> Self {
        self.config.app_identifier = app.into();
        self
    }

    /// Add a URL query parameter.
    pub fn add_url_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.url_params.insert(key.into(), value.into());
        self
    }

    /// Add an extra header.
    pub fn add_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.extra_headers.insert(key.into(), value.into());
        self
    }

    /// Build the configuration.
    pub fn build(self) -> OAuthConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OAuthConfig::default();
        assert_eq!(config.system_prompt, DEFAULT_SYSTEM_PROMPT);
        assert_eq!(config.beta_flags.len(), 3);
        assert_eq!(config.user_agent, DEFAULT_USER_AGENT);
        assert_eq!(config.app_identifier, DEFAULT_APP_IDENTIFIER);
    }

    #[test]
    fn test_builder() {
        let config = OAuthConfig::builder()
            .system_prompt("Custom prompt")
            .user_agent("my-app/1.0")
            .add_beta_flag("new-flag")
            .build();

        assert_eq!(config.system_prompt, "Custom prompt");
        assert_eq!(config.user_agent, "my-app/1.0");
        assert!(config.beta_flags.contains(&"new-flag".to_string()));
    }

    #[test]
    fn test_beta_header_value() {
        let config = OAuthConfig::default();
        assert_eq!(
            config.beta_header_value(),
            "claude-code-20250219,oauth-2025-04-20,interleaved-thinking-2025-05-14"
        );
    }
}
