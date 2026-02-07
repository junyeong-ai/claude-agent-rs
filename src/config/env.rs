//! Environment Variable Configuration Provider
//!
//! Provides read-only access to configuration via environment variables.
//! Environment variables are immutable at runtime for thread-safety.

use super::provider::ConfigProvider;
use super::{ConfigError, ConfigResult};

/// Read-only environment variable configuration provider.
///
/// Environment variables are treated as immutable at runtime because
/// modifying them is not thread-safe (requires unsafe in Rust 1.80+).
#[derive(Debug, Clone)]
pub struct EnvConfigProvider {
    prefix: Option<String>,
}

impl EnvConfigProvider {
    /// Create a new environment provider with no prefix
    pub fn new() -> Self {
        Self { prefix: None }
    }

    /// Create an environment provider with a prefix
    pub fn prefixed(prefix: impl Into<String>) -> Self {
        Self {
            prefix: Some(prefix.into()),
        }
    }

    /// Get the full environment variable name
    fn env_key(&self, key: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}{}", prefix, key.to_uppercase().replace('.', "_")),
            None => key.to_uppercase().replace('.', "_"),
        }
    }

    /// Reverse: extract key from environment variable name
    fn key_from_env(&self, env_name: &str) -> Option<String> {
        match &self.prefix {
            Some(prefix) => {
                if env_name.starts_with(prefix) {
                    Some(env_name[prefix.len()..].to_lowercase().replace('_', "."))
                } else {
                    None
                }
            }
            None => Some(env_name.to_lowercase().replace('_', ".")),
        }
    }
}

impl Default for EnvConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ConfigProvider for EnvConfigProvider {
    fn name(&self) -> &str {
        "env"
    }

    async fn get_raw(&self, key: &str) -> ConfigResult<Option<String>> {
        let env_key = self.env_key(key);
        match std::env::var(&env_key) {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(e) => Err(ConfigError::Env(e)),
        }
    }

    async fn set_raw(&self, _key: &str, _value: &str) -> ConfigResult<()> {
        Err(ConfigError::Provider {
            message: "Environment variables are read-only at runtime".into(),
        })
    }

    async fn delete(&self, _key: &str) -> ConfigResult<bool> {
        Err(ConfigError::Provider {
            message: "Environment variables are read-only at runtime".into(),
        })
    }

    async fn list_keys(&self, prefix: &str) -> ConfigResult<Vec<String>> {
        let env_prefix = self.env_key(prefix);
        let keys: Vec<String> = std::env::vars()
            .filter_map(|(k, _)| {
                if k.starts_with(&env_prefix) {
                    self.key_from_env(&k)
                } else {
                    None
                }
            })
            .collect();
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_env_key_conversion() {
        let provider = EnvConfigProvider::new();
        assert_eq!(provider.env_key("api.key"), "API_KEY");
        assert_eq!(provider.env_key("model.name"), "MODEL_NAME");

        let provider = EnvConfigProvider::prefixed("CLAUDE_");
        assert_eq!(provider.env_key("api.key"), "CLAUDE_API_KEY");
    }

    #[tokio::test]
    async fn test_env_provider_get() {
        let provider = EnvConfigProvider::prefixed("TEST_CONFIG_");

        // SAFETY: Test-only environment setup
        unsafe { std::env::set_var("TEST_CONFIG_MY_KEY", "my_value") };
        let value = provider.get_raw("my.key").await.unwrap();
        assert_eq!(value, Some("my_value".to_string()));
        unsafe { std::env::remove_var("TEST_CONFIG_MY_KEY") };
    }

    #[tokio::test]
    async fn test_env_provider_read_only() {
        let provider = EnvConfigProvider::new();

        // set_raw should fail (read-only)
        assert!(provider.set_raw("key", "value").await.is_err());

        // delete should fail (read-only)
        assert!(provider.delete("key").await.is_err());
    }

    #[tokio::test]
    async fn test_env_provider_not_found() {
        let provider = EnvConfigProvider::prefixed("NONEXISTENT_PREFIX_");
        let value = provider.get_raw("some.key").await.unwrap();
        assert_eq!(value, None);
    }
}
