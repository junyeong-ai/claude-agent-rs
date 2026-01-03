use std::collections::HashMap;

use async_trait::async_trait;

use super::{Named, SourceType};

#[async_trait]
pub trait Provider<T: Named + Clone + Send + Sync>: Send + Sync {
    fn provider_name(&self) -> &str;
    fn priority(&self) -> i32 {
        0
    }
    fn source_type(&self) -> SourceType {
        SourceType::User
    }

    async fn list(&self) -> crate::Result<Vec<String>>;
    async fn get(&self, name: &str) -> crate::Result<Option<T>>;
    async fn load_all(&self) -> crate::Result<Vec<T>>;
}

#[derive(Debug, Clone)]
pub struct InMemoryProvider<T> {
    items: HashMap<String, T>,
    priority: i32,
    source_type: SourceType,
}

impl<T: Named + Clone + Send + Sync> Default for InMemoryProvider<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Named + Clone + Send + Sync> InMemoryProvider<T> {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            priority: 0,
            source_type: SourceType::User,
        }
    }

    pub fn with_item(mut self, item: T) -> Self {
        self.add(item);
        self
    }

    pub fn add(&mut self, item: T) {
        self.items.insert(item.name().to_string(), item);
    }

    pub fn with_items(mut self, items: impl IntoIterator<Item = T>) -> Self {
        for item in items {
            self.add(item);
        }
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_source_type(mut self, source_type: SourceType) -> Self {
        self.source_type = source_type;
        self
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[async_trait]
impl<T: Named + Clone + Send + Sync + 'static> Provider<T> for InMemoryProvider<T> {
    fn provider_name(&self) -> &str {
        "in-memory"
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn source_type(&self) -> SourceType {
        self.source_type
    }

    async fn list(&self) -> crate::Result<Vec<String>> {
        Ok(self.items.keys().cloned().collect())
    }

    async fn get(&self, name: &str) -> crate::Result<Option<T>> {
        Ok(self.items.get(name).cloned())
    }

    async fn load_all(&self) -> crate::Result<Vec<T>> {
        Ok(self.items.values().cloned().collect())
    }
}

#[derive(Default)]
pub struct ChainProvider<T: Named + Clone + Send + Sync + 'static> {
    providers: Vec<Box<dyn Provider<T>>>,
}

impl<T: Named + Clone + Send + Sync + 'static> ChainProvider<T> {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn with(mut self, provider: impl Provider<T> + 'static) -> Self {
        self.providers.push(Box::new(provider));
        self
    }

    pub fn add(&mut self, provider: impl Provider<T> + 'static) {
        self.providers.push(Box::new(provider));
    }
}

#[async_trait]
impl<T: Named + Clone + Send + Sync + 'static> Provider<T> for ChainProvider<T> {
    fn provider_name(&self) -> &str {
        "chain"
    }

    fn priority(&self) -> i32 {
        self.providers
            .iter()
            .map(|p| p.priority())
            .max()
            .unwrap_or(0)
    }

    async fn list(&self) -> crate::Result<Vec<String>> {
        let mut all = Vec::new();
        for p in &self.providers {
            all.extend(p.list().await?);
        }
        all.sort();
        all.dedup();
        Ok(all)
    }

    async fn get(&self, name: &str) -> crate::Result<Option<T>> {
        let mut sorted: Vec<_> = self.providers.iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.priority()));

        for provider in sorted {
            if let Some(item) = provider.get(name).await? {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }

    async fn load_all(&self) -> crate::Result<Vec<T>> {
        let mut map: HashMap<String, T> = HashMap::new();

        let mut sorted: Vec<_> = self.providers.iter().collect();
        sorted.sort_by_key(|p| std::cmp::Reverse(p.priority()));

        for provider in sorted {
            for item in provider.load_all().await? {
                map.entry(item.name().to_string()).or_insert(item);
            }
        }

        Ok(map.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestItem {
        name: String,
        value: i32,
    }

    impl Named for TestItem {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_in_memory_provider() {
        let provider = InMemoryProvider::new()
            .with_item(TestItem {
                name: "a".into(),
                value: 1,
            })
            .with_item(TestItem {
                name: "b".into(),
                value: 2,
            });

        assert_eq!(provider.len(), 2);

        let names = provider.list().await.unwrap();
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));

        let item = provider.get("a").await.unwrap().unwrap();
        assert_eq!(item.value, 1);

        assert!(provider.get("nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_chain_provider_priority() {
        let low = InMemoryProvider::new()
            .with_item(TestItem {
                name: "shared".into(),
                value: 1,
            })
            .with_priority(0);

        let high = InMemoryProvider::new()
            .with_item(TestItem {
                name: "shared".into(),
                value: 10,
            })
            .with_priority(10);

        let chain = ChainProvider::new().with(low).with(high);

        let item = chain.get("shared").await.unwrap().unwrap();
        assert_eq!(item.value, 10);
    }

    #[tokio::test]
    async fn test_chain_provider_load_all() {
        let p1 = InMemoryProvider::new()
            .with_item(TestItem {
                name: "a".into(),
                value: 1,
            })
            .with_priority(0);

        let p2 = InMemoryProvider::new()
            .with_item(TestItem {
                name: "b".into(),
                value: 2,
            })
            .with_priority(10);

        let chain = ChainProvider::new().with(p1).with(p2);

        let items = chain.load_all().await.unwrap();
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_chain_provider_load_all_priority_order() {
        let low = InMemoryProvider::new()
            .with_item(TestItem {
                name: "shared".into(),
                value: 1,
            })
            .with_priority(0);

        let high = InMemoryProvider::new()
            .with_item(TestItem {
                name: "shared".into(),
                value: 100,
            })
            .with_priority(10);

        let chain = ChainProvider::new().with(low).with(high);

        let items = chain.load_all().await.unwrap();
        assert_eq!(items.len(), 1);

        let item = items.into_iter().find(|i| i.name == "shared").unwrap();
        assert_eq!(item.value, 100, "High priority item should be kept");
    }
}
