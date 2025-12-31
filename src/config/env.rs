//! Environment Variable Configuration Provider
//!
//! Loads configuration from environment variables with optional prefix.

use super::provider::ConfigProvider;
use super::{ConfigError, ConfigResult};

/// Environment variable configuration provider
#[derive(Debug, Clone)]
pub struct EnvConfigProvider {
    /// Prefix for environment variables (e.g., "CLAUDE_")
    prefix: Option<String>,
}

impl EnvConfigProvider {
    /// Create a new environment provider with no prefix
    pub fn new() -> Self {
        Self { prefix: None }
    }

    /// Create an environment provider with a prefix
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
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
                    Some(
                        env_name[prefix.len()..]
                            .to_lowercase()
                            .replace('_', "."),
                    )
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

    async fn set_raw(&self, key: &str, value: &str) -> ConfigResult<()> {
        let env_key = self.env_key(key);
        std::env::set_var(&env_key, value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> ConfigResult<bool> {
        let env_key = self.env_key(key);
        let existed = std::env::var(&env_key).is_ok();
        if existed {
            std::env::remove_var(&env_key);
        }
        Ok(existed)
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

        let provider = EnvConfigProvider::with_prefix("CLAUDE_");
        assert_eq!(provider.env_key("api.key"), "CLAUDE_API_KEY");
    }

    #[tokio::test]
    async fn test_env_provider_get_set() {
        let provider = EnvConfigProvider::with_prefix("TEST_CONFIG_");

        // Set via env
        std::env::set_var("TEST_CONFIG_MY_KEY", "my_value");

        // Get via provider
        let value = provider.get_raw("my.key").await.unwrap();
        assert_eq!(value, Some("my_value".to_string()));

        // Cleanup
        std::env::remove_var("TEST_CONFIG_MY_KEY");
    }

    #[tokio::test]
    async fn test_env_provider_set_and_delete() {
        let provider = EnvConfigProvider::with_prefix("TEST_DEL_");

        // Set via provider
        provider.set_raw("temp.key", "temp_value").await.unwrap();
        assert_eq!(
            provider.get_raw("temp.key").await.unwrap(),
            Some("temp_value".to_string())
        );

        // Delete
        assert!(provider.delete("temp.key").await.unwrap());
        assert_eq!(provider.get_raw("temp.key").await.unwrap(), None);

        // Delete non-existent
        assert!(!provider.delete("temp.key").await.unwrap());
    }

    #[tokio::test]
    async fn test_env_provider_not_found() {
        let provider = EnvConfigProvider::with_prefix("NONEXISTENT_PREFIX_");

        let value = provider.get_raw("some.key").await.unwrap();
        assert_eq!(value, None);
    }
}
