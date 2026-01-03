//! Generic registry for managing named items.

use std::collections::HashMap;
use std::path::Path;

use super::{DocumentLoader, Named, SourceType};

pub trait RegistryItem: Named + Clone + Send + Sync {
    fn source_type(&self) -> SourceType;
}

#[derive(Debug, Clone)]
pub struct BaseRegistry<T, L>
where
    T: RegistryItem,
    L: DocumentLoader<T>,
{
    items: HashMap<String, T>,
    loader: L,
}

impl<T, L> BaseRegistry<T, L>
where
    T: RegistryItem + 'static,
    L: DocumentLoader<T> + Default,
{
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            loader: L::default(),
        }
    }
}

impl<T, L> Default for BaseRegistry<T, L>
where
    T: RegistryItem + 'static,
    L: DocumentLoader<T> + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, L> BaseRegistry<T, L>
where
    T: RegistryItem + 'static,
    L: DocumentLoader<T>,
{
    pub fn with_loader(loader: L) -> Self {
        Self {
            items: HashMap::new(),
            loader,
        }
    }

    pub fn register(&mut self, item: T) {
        self.items.insert(item.name().to_string(), item);
    }

    pub fn get(&self, name: &str) -> Option<&T> {
        self.items.get(name)
    }

    pub fn list(&self) -> Vec<&str> {
        self.items.keys().map(String::as_str).collect()
    }

    pub fn items(&self) -> impl Iterator<Item = &T> {
        self.items.values()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.items.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<T> {
        self.items.remove(name)
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub async fn load_file(&mut self, path: &Path) -> crate::Result<()> {
        let item = self.loader.load_file(path).await?;
        self.register(item);
        Ok(())
    }

    pub async fn load_directory(&mut self, dir: &Path) -> crate::Result<usize> {
        let items = self.loader.load_directory(dir).await?;
        let count = items.len();
        for item in items {
            self.register(item);
        }
        Ok(count)
    }

    pub fn load_inline(&mut self, content: &str) -> crate::Result<()> {
        let item = self.loader.parse_content(content, None)?;
        self.register(item);
        Ok(())
    }

    pub fn get_by_source(&self, source_type: SourceType) -> Vec<&T> {
        self.items
            .values()
            .filter(|item| item.source_type() == source_type)
            .collect()
    }

    pub fn loader(&self) -> &L {
        &self.loader
    }

    pub fn register_all(&mut self, items: impl IntoIterator<Item = T>) {
        for item in items {
            self.register(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestItem {
        name: String,
        source_type: SourceType,
    }

    impl TestItem {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                source_type: SourceType::User,
            }
        }

        fn builtin(name: &str) -> Self {
            Self {
                name: name.to_string(),
                source_type: SourceType::Builtin,
            }
        }
    }

    impl Named for TestItem {
        fn name(&self) -> &str {
            &self.name
        }
    }

    impl RegistryItem for TestItem {
        fn source_type(&self) -> SourceType {
            self.source_type
        }
    }

    #[derive(Debug, Clone, Default)]
    struct TestLoader;

    impl DocumentLoader<TestItem> for TestLoader {
        fn parse_content(&self, content: &str, _path: Option<&Path>) -> crate::Result<TestItem> {
            Ok(TestItem::new(content.trim()))
        }

        fn doc_type_name(&self) -> &'static str {
            "test"
        }

        fn file_filter(&self) -> fn(&Path) -> bool {
            |p| p.extension().is_some_and(|e| e == "md")
        }
    }

    type TestRegistry = BaseRegistry<TestItem, TestLoader>;

    #[test]
    fn test_basic_operations() {
        let mut registry = TestRegistry::new();

        registry.register(TestItem::new("item1"));
        registry.register(TestItem::new("item2"));

        assert_eq!(registry.len(), 2);
        assert!(registry.get("item1").is_some());
        assert!(registry.get("nonexistent").is_none());
        assert!(registry.contains("item1"));
        assert!(!registry.contains("nonexistent"));
    }

    #[test]
    fn test_list() {
        let mut registry = TestRegistry::new();
        registry.register(TestItem::new("a"));
        registry.register(TestItem::new("b"));

        let names = registry.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn test_remove() {
        let mut registry = TestRegistry::new();
        registry.register(TestItem::new("test"));

        assert!(registry.remove("test").is_some());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_get_by_source() {
        let mut registry = TestRegistry::new();
        registry.register(TestItem::new("user1"));
        registry.register(TestItem::builtin("builtin1"));
        registry.register(TestItem::builtin("builtin2"));

        let users = registry.get_by_source(SourceType::User);
        let builtins = registry.get_by_source(SourceType::Builtin);

        assert_eq!(users.len(), 1);
        assert_eq!(builtins.len(), 2);
    }

    #[test]
    fn test_load_inline() {
        let mut registry = TestRegistry::new();
        registry.load_inline("inline-item").unwrap();
        assert!(registry.get("inline-item").is_some());
    }

    #[test]
    fn test_register_all() {
        let mut registry = TestRegistry::new();
        registry.register_all([TestItem::new("a"), TestItem::new("b"), TestItem::new("c")]);
        assert_eq!(registry.len(), 3);
    }
}
