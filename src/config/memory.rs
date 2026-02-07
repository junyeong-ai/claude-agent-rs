//! In-Memory Configuration Provider
//!
//! Provides a simple in-memory key-value store for configuration.
//! Useful for testing and code-defined configuration.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::ConfigResult;
use super::provider::ConfigProvider;

/// In-memory configuration provider
#[derive(Debug, Default)]
pub struct MemoryConfigProvider {
    data: Arc<RwLock<HashMap<String, String>>>,
    name: String,
}

impl MemoryConfigProvider {
    /// Create a new empty memory provider
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            name: "memory".to_string(),
        }
    }

    /// Create a memory provider with a custom name
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            name: name.into(),
        }
    }

    /// Create a memory provider with initial data
    pub fn from_data(data: HashMap<String, String>) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            name: "memory".to_string(),
        }
    }

    /// Add an initial value during construction (builder pattern)
    pub fn value(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        // Safe: called during construction before any async access.
        // Arc::get_mut fails only if the Arc has been cloned, which cannot happen
        // during builder-style construction.
        Arc::get_mut(&mut self.data)
            .expect(
                "MemoryConfigProvider::value() called after Arc was shared; use insert() instead",
            )
            .get_mut()
            .insert(key.into(), value.into());
        self
    }

    /// Insert a value asynchronously
    pub async fn insert(&self, key: impl Into<String>, value: impl Into<String>) {
        let mut data = self.data.write().await;
        data.insert(key.into(), value.into());
    }

    /// Get the number of stored values
    pub async fn len(&self) -> usize {
        self.data.read().await.len()
    }

    /// Check if empty
    pub async fn is_empty(&self) -> bool {
        self.data.read().await.is_empty()
    }

    /// Clear all values
    pub async fn clear(&self) {
        self.data.write().await.clear();
    }
}

#[async_trait::async_trait]
impl ConfigProvider for MemoryConfigProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn get_raw(&self, key: &str) -> ConfigResult<Option<String>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set_raw(&self, key: &str, value: &str) -> ConfigResult<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete(&self, key: &str) -> ConfigResult<bool> {
        let mut data = self.data.write().await;
        Ok(data.remove(key).is_some())
    }

    async fn list_keys(&self, prefix: &str) -> ConfigResult<Vec<String>> {
        let data = self.data.read().await;
        let keys: Vec<String> = data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_provider_basic() {
        let provider = MemoryConfigProvider::new();

        // Set and get
        provider.set_raw("key1", "value1").await.unwrap();
        let value = provider.get_raw("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // Get non-existent
        let value = provider.get_raw("nonexistent").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_memory_provider_delete() {
        let provider = MemoryConfigProvider::new();

        provider.set_raw("key1", "value1").await.unwrap();
        assert!(provider.delete("key1").await.unwrap());
        assert!(!provider.delete("key1").await.unwrap());

        let value = provider.get_raw("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_memory_provider_list_keys() {
        let provider = MemoryConfigProvider::new();

        provider.set_raw("app.name", "test").await.unwrap();
        provider.set_raw("app.version", "1.0").await.unwrap();
        provider.set_raw("other.key", "value").await.unwrap();

        let keys = provider.list_keys("app.").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"app.name".to_string()));
        assert!(keys.contains(&"app.version".to_string()));
    }

    #[tokio::test]
    async fn test_memory_provider_typed() {
        use crate::config::provider::ConfigProviderExt;

        let provider = MemoryConfigProvider::new();

        // Set typed value
        ConfigProviderExt::set(&provider, "count", &42i32)
            .await
            .unwrap();

        // Get typed value
        let count: Option<i32> = ConfigProviderExt::get(&provider, "count").await.unwrap();
        assert_eq!(count, Some(42));
    }

    #[tokio::test]
    async fn test_memory_provider_with_data() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        data.insert("key2".to_string(), "value2".to_string());

        let provider = MemoryConfigProvider::from_data(data);

        assert_eq!(provider.len().await, 2);
        assert_eq!(
            provider.get_raw("key1").await.unwrap(),
            Some("value1".to_string())
        );
    }
}
