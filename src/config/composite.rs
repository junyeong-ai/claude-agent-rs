//! Composite Configuration Provider
//!
//! Chains multiple configuration providers with priority ordering.
//! Earlier providers have higher priority.

use super::ConfigResult;
use super::provider::ConfigProvider;

/// Composite configuration provider that chains multiple providers
pub struct CompositeConfigProvider {
    providers: Vec<Box<dyn ConfigProvider>>,
}

impl CompositeConfigProvider {
    /// Create a new empty composite provider
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add a provider (first added = highest priority)
    pub fn add_provider(&mut self, provider: Box<dyn ConfigProvider>) {
        self.providers.push(provider);
    }

    /// Add a provider and return self (for chaining)
    pub fn provider(mut self, provider: Box<dyn ConfigProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    /// Get the number of providers
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Get provider names
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }
}

impl Default for CompositeConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ConfigProvider for CompositeConfigProvider {
    fn name(&self) -> &str {
        "composite"
    }

    async fn get_raw(&self, key: &str) -> ConfigResult<Option<String>> {
        // Try each provider in order (first match wins)
        for provider in &self.providers {
            if let Some(value) = provider.get_raw(key).await? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    async fn set_raw(&self, key: &str, value: &str) -> ConfigResult<()> {
        // Set in the first provider (highest priority)
        if let Some(provider) = self.providers.first() {
            provider.set_raw(key, value).await?;
        }
        Ok(())
    }

    async fn delete(&self, key: &str) -> ConfigResult<bool> {
        // Delete from all providers that have the key
        let mut deleted = false;
        for provider in &self.providers {
            if provider.delete(key).await? {
                deleted = true;
            }
        }
        Ok(deleted)
    }

    async fn list_keys(&self, prefix: &str) -> ConfigResult<Vec<String>> {
        // Collect unique keys from all providers
        let mut all_keys = std::collections::HashSet::new();
        for provider in &self.providers {
            for key in provider.list_keys(prefix).await? {
                all_keys.insert(key);
            }
        }
        Ok(all_keys.into_iter().collect())
    }
}

impl std::fmt::Debug for CompositeConfigProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeConfigProvider")
            .field("provider_count", &self.providers.len())
            .field("provider_names", &self.provider_names())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::memory::MemoryConfigProvider;

    #[tokio::test]
    async fn test_composite_provider_priority() {
        let high_priority = MemoryConfigProvider::named("high");
        high_priority.set_raw("key", "high_value").await.unwrap();

        let low_priority = MemoryConfigProvider::named("low");
        low_priority.set_raw("key", "low_value").await.unwrap();
        low_priority.set_raw("only_low", "from_low").await.unwrap();

        let composite = CompositeConfigProvider::new()
            .provider(Box::new(high_priority))
            .provider(Box::new(low_priority));

        // High priority wins
        assert_eq!(
            composite.get_raw("key").await.unwrap(),
            Some("high_value".to_string())
        );

        // Falls through to low priority
        assert_eq!(
            composite.get_raw("only_low").await.unwrap(),
            Some("from_low".to_string())
        );
    }

    #[tokio::test]
    async fn test_composite_provider_list_keys() {
        let p1 = MemoryConfigProvider::new();
        p1.set_raw("app.name", "value1").await.unwrap();
        p1.set_raw("app.version", "value2").await.unwrap();

        let p2 = MemoryConfigProvider::new();
        p2.set_raw("app.config", "value3").await.unwrap();
        p2.set_raw("other", "value4").await.unwrap();

        let composite = CompositeConfigProvider::new()
            .provider(Box::new(p1))
            .provider(Box::new(p2));

        let keys = composite.list_keys("app.").await.unwrap();
        assert_eq!(keys.len(), 3); // name, version, config
    }

    #[tokio::test]
    async fn test_composite_provider_set() {
        let composite = CompositeConfigProvider::new()
            .provider(Box::new(MemoryConfigProvider::new()))
            .provider(Box::new(MemoryConfigProvider::new()));

        composite.set_raw("new_key", "new_value").await.unwrap();

        // Should be readable
        assert_eq!(
            composite.get_raw("new_key").await.unwrap(),
            Some("new_value".to_string())
        );
    }

    #[tokio::test]
    async fn test_composite_provider_delete() {
        let p1 = MemoryConfigProvider::new();
        p1.set_raw("shared", "from_p1").await.unwrap();

        let p2 = MemoryConfigProvider::new();
        p2.set_raw("shared", "from_p2").await.unwrap();

        let composite = CompositeConfigProvider::new()
            .provider(Box::new(p1))
            .provider(Box::new(p2));

        // Delete from all
        assert!(composite.delete("shared").await.unwrap());
        assert_eq!(composite.get_raw("shared").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_composite_provider_names() {
        let composite = CompositeConfigProvider::new()
            .provider(Box::new(MemoryConfigProvider::named("first")))
            .provider(Box::new(MemoryConfigProvider::named("second")));

        let names = composite.provider_names();
        assert_eq!(names, vec!["first", "second"]);
    }
}
