//! Configuration Provider Trait

use serde::{Serialize, de::DeserializeOwned};

use super::ConfigResult;

/// Core configuration provider trait
#[async_trait::async_trait]
pub trait ConfigProvider: Send + Sync {
    /// Provider name for logging
    fn name(&self) -> &str;

    /// Get a raw configuration value
    async fn get_raw(&self, key: &str) -> ConfigResult<Option<String>>;

    /// Set a raw configuration value
    async fn set_raw(&self, key: &str, value: &str) -> ConfigResult<()>;

    /// Delete a configuration value
    async fn delete(&self, key: &str) -> ConfigResult<bool>;

    /// List keys matching a prefix
    async fn list_keys(&self, prefix: &str) -> ConfigResult<Vec<String>>;
}

/// Extension methods for typed configuration access
pub trait ConfigProviderExt: ConfigProvider {
    /// Get a typed configuration value
    fn get<T: DeserializeOwned + Send>(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = ConfigResult<Option<T>>> + Send
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
    fn set<T: Serialize + Send + Sync>(
        &self,
        key: &str,
        value: &T,
    ) -> impl std::future::Future<Output = ConfigResult<()>> + Send
    where
        Self: Sync,
    {
        async move {
            let raw = serde_json::to_string(value)?;
            self.set_raw(key, &raw).await
        }
    }
}

impl<P: ConfigProvider + ?Sized> ConfigProviderExt for P {}
