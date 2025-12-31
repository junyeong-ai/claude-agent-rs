//! Configuration Provider Trait
//!
//! Defines the core trait for configuration providers.

use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc;

use super::ConfigResult;

/// Core configuration provider trait
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    /// Provider name for logging
    fn name(&self) -> &str;

    /// Get a configuration value by key
    async fn get_raw(&self, key: &str) -> ConfigResult<Option<String>>;

    /// Set a configuration value
    async fn set_raw(&self, key: &str, value: &str) -> ConfigResult<()>;

    /// Delete a configuration value
    async fn delete(&self, key: &str) -> ConfigResult<bool>;

    /// List all keys matching a prefix
    async fn list_keys(&self, prefix: &str) -> ConfigResult<Vec<String>>;
}

/// Extension methods for typed configuration access
pub trait ConfigProviderExt: ConfigProvider {
    /// Get a typed configuration value
    fn get<T: DeserializeOwned + Send>(&self, key: &str) -> impl std::future::Future<Output = ConfigResult<Option<T>>> + Send
    where
        Self: Sync,
    {
        async move {
            match self.get_raw(key).await? {
                Some(raw) => {
                    let value: T = serde_json::from_str(&raw).map_err(|e| {
                        super::ConfigError::InvalidValue {
                            key: key.to_string(),
                            message: e.to_string(),
                        }
                    })?;
                    Ok(Some(value))
                }
                None => Ok(None),
            }
        }
    }

    /// Set a typed configuration value
    fn set<T: Serialize + Send + Sync>(&self, key: &str, value: &T) -> impl std::future::Future<Output = ConfigResult<()>> + Send
    where
        Self: Sync,
    {
        async move {
            let raw = serde_json::to_string(value)?;
            self.set_raw(key, &raw).await
        }
    }
}

// Blanket implementation
impl<P: ConfigProvider + ?Sized> ConfigProviderExt for P {}

/// A configuration change event
#[derive(Clone, Debug)]
pub struct ConfigChange {
    /// The key that changed
    pub key: String,
    /// The new value (None if deleted)
    pub value: Option<String>,
    /// The change type
    pub change_type: ConfigChangeType,
}

/// Type of configuration change
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigChangeType {
    /// Value was created
    Created,
    /// Value was updated
    Updated,
    /// Value was deleted
    Deleted,
}

/// Extension trait for watchable configuration providers
#[async_trait::async_trait]
pub trait WatchableConfig: ConfigProvider {
    /// Watch for configuration changes matching a pattern
    async fn watch(&self, pattern: &str) -> ConfigResult<mpsc::Receiver<ConfigChange>>;
}

/// Extension trait for tenant-aware configuration providers
#[async_trait::async_trait]
pub trait TenantAwareConfig: ConfigProvider {
    /// Get a tenant-scoped provider
    fn with_tenant(&self, tenant_id: &str) -> Box<dyn ConfigProvider>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_change_types() {
        let change = ConfigChange {
            key: "test".to_string(),
            value: Some("value".to_string()),
            change_type: ConfigChangeType::Created,
        };
        assert_eq!(change.change_type, ConfigChangeType::Created);
    }
}
