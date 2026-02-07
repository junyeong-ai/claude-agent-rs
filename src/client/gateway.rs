//! LLM Gateway configuration for custom proxy/endpoint support.

use std::env;

/// LLM Gateway configuration.
///
/// Provides a centralized proxy layer between the SDK and model providers,
/// supporting custom endpoints, authentication, and header injection.
#[derive(Clone, Debug, Default)]
pub struct GatewayConfig {
    /// Custom base URL (overrides provider URL)
    pub base_url: Option<String>,
    /// Custom auth token (overrides provider auth)
    pub auth_token: Option<String>,
    /// Custom headers (key-value pairs)
    pub custom_headers: Vec<(String, String)>,
    /// Disable experimental beta flags for gateway compatibility
    pub disable_betas: bool,
}

impl GatewayConfig {
    /// Create from environment variables.
    pub fn from_env() -> Option<Self> {
        let base_url = env::var("ANTHROPIC_BASE_URL").ok();
        let auth_token = env::var("ANTHROPIC_AUTH_TOKEN").ok();
        let custom_headers = parse_custom_headers();
        let disable_betas = env::var("CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if base_url.is_some() || auth_token.is_some() || !custom_headers.is_empty() {
            Some(Self {
                base_url,
                auth_token,
                custom_headers,
                disable_betas,
            })
        } else {
            None
        }
    }

    /// Create with custom base URL.
    pub fn base_url(url: impl Into<String>) -> Self {
        Self {
            base_url: Some(url.into()),
            ..Default::default()
        }
    }

    /// Set auth token.
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Add a custom header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.push((name.into(), value.into()));
        self
    }

    /// Add multiple headers.
    pub fn headers(mut self, headers: impl IntoIterator<Item = (String, String)>) -> Self {
        self.custom_headers.extend(headers);
        self
    }

    /// Disable experimental beta flags.
    pub fn disable_betas(mut self) -> Self {
        self.disable_betas = true;
        self
    }

    /// Check if gateway is configured.
    pub fn is_active(&self) -> bool {
        self.base_url.is_some() || self.auth_token.is_some()
    }

    /// Get effective base URL.
    pub fn effective_base_url(&self, default: &str) -> String {
        self.base_url.clone().unwrap_or_else(|| default.to_string())
    }

    /// Get all headers including custom ones.
    pub fn all_headers(&self) -> Vec<(String, String)> {
        self.custom_headers.clone()
    }
}

/// Parse ANTHROPIC_CUSTOM_HEADERS environment variable.
/// Format: "Header1: Value1\nHeader2: Value2"
fn parse_custom_headers() -> Vec<(String, String)> {
    env::var("ANTHROPIC_CUSTOM_HEADERS")
        .ok()
        .map(|s| {
            s.lines()
                .filter_map(|line| {
                    let mut parts = line.splitn(2, ':');
                    match (parts.next(), parts.next()) {
                        (Some(key), Some(value)) => {
                            Some((key.trim().to_string(), value.trim().to_string()))
                        }
                        _ => None,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config_builder() {
        let config = GatewayConfig::base_url("https://my-gateway.com")
            .auth_token("my-token")
            .header("X-Custom", "value")
            .disable_betas();

        assert_eq!(config.base_url, Some("https://my-gateway.com".to_string()));
        assert_eq!(config.auth_token, Some("my-token".to_string()));
        assert!(config.disable_betas);
        assert_eq!(config.custom_headers.len(), 1);
    }

    #[test]
    fn test_effective_base_url() {
        let config = GatewayConfig::default();
        assert_eq!(
            config.effective_base_url("https://default.com"),
            "https://default.com"
        );

        let config = GatewayConfig::base_url("https://custom.com");
        assert_eq!(
            config.effective_base_url("https://default.com"),
            "https://custom.com"
        );
    }
}
