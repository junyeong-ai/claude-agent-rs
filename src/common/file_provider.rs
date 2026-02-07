//! Generic file-based provider for loading items from the filesystem.
//!
//! This module provides a reusable `FileProvider<T, L>` that can be used to load
//! any type T from markdown files using a configurable loader L.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{Named, Provider, SourceType, load_files};

/// Trait for loading items from files.
///
/// Implementors only need to implement `parse_content()`, `doc_type_name()`, and `file_filter()`.
/// The `load_file()` and `load_directory()` methods have default implementations.
#[async_trait]
pub trait DocumentLoader<T: Send>: Clone + Send + Sync {
    /// Parse content into the target type.
    fn parse_content(&self, content: &str, path: Option<&Path>) -> crate::Result<T>;

    /// Document type name for error messages (e.g., "skill", "subagent", "output style").
    fn doc_type_name(&self) -> &'static str;

    /// File filter function for directory loading.
    fn file_filter(&self) -> fn(&Path) -> bool;

    /// Load an item from a file path.
    async fn load_file(&self, path: &Path) -> crate::Result<T> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            crate::Error::Config(format!(
                "Failed to read {} file {}: {}",
                self.doc_type_name(),
                path.display(),
                e
            ))
        })?;
        self.parse_content(&content, Some(path))
    }

    /// Load all items from a directory.
    async fn load_directory(&self, dir: &Path) -> crate::Result<Vec<T>>
    where
        T: 'static,
    {
        let loader = self.clone();
        let filter = self.file_filter();
        load_files(dir, filter, move |p| {
            let l = loader.clone();
            async move { l.load_file(&p).await }
        })
        .await
    }

    /// Load from inline content.
    fn load_inline(&self, content: &str) -> crate::Result<T> {
        self.parse_content(content, None)
    }
}

/// Trait for defining file lookup strategies.
///
/// Different item types may have different file naming conventions.
/// For example, skills use `SKILL.md` in subdirectories or `*.skill.md`,
/// while subagents use simple `*.md` files.
pub trait LookupStrategy: Clone + Send + Sync {
    /// Get the subdirectory name within `.claude/` for this item type.
    /// e.g., "skills" or "agents"
    fn config_subdir(&self) -> &'static str;

    /// Try to find a file for a specific item name in a directory.
    /// Returns the file path if found, None otherwise.
    fn find_by_name(&self, dir: &Path, name: &str) -> Option<PathBuf>;
}

fn find_markdown_by_name(dir: &Path, name: &str) -> Option<PathBuf> {
    let file = dir.join(format!("{}.md", name));
    file.exists().then_some(file)
}

/// Lookup strategy for output styles (`<name>.md` in `.claude/output-styles/`).
#[derive(Debug, Clone, Copy, Default)]
pub struct OutputStyleLookupStrategy;

impl LookupStrategy for OutputStyleLookupStrategy {
    fn config_subdir(&self) -> &'static str {
        "output-styles"
    }

    fn find_by_name(&self, dir: &Path, name: &str) -> Option<PathBuf> {
        find_markdown_by_name(dir, name)
    }
}

/// Generic file-based provider for loading items from the filesystem.
///
/// This provider supports:
/// - Multiple search paths with priority ordering
/// - Configurable lookup strategies for different file patterns
/// - Builder pattern for configuration
///
/// # Type Parameters
///
/// - `T`: The item type to load (must implement `Named + Clone + Send + Sync`)
/// - `L`: The loader type (must implement `DocumentLoader<T>`)
/// - `S`: The lookup strategy (must implement `LookupStrategy`)
///
/// # Example
///
/// ```ignore
/// let provider = FileProvider::new(StyleLoader::new(), OutputStyleLookupStrategy)
///     .project_path(project_dir)
///     .user_path()
///     .priority(10);
/// ```
pub struct FileProvider<T, L, S>
where
    T: Named + Clone + Send + Sync,
    L: DocumentLoader<T>,
    S: LookupStrategy,
{
    paths: Vec<PathBuf>,
    priority: i32,
    source_type: SourceType,
    loader: L,
    strategy: S,
    _marker: std::marker::PhantomData<T>,
}

impl<T, L, S> FileProvider<T, L, S>
where
    T: Named + Clone + Send + Sync,
    L: DocumentLoader<T>,
    S: LookupStrategy,
{
    /// Create a new file provider with the given loader and lookup strategy.
    pub fn new(loader: L, strategy: S) -> Self {
        Self {
            paths: Vec::new(),
            priority: 0,
            source_type: SourceType::Project,
            loader,
            strategy,
            _marker: std::marker::PhantomData,
        }
    }

    /// Add a path to search for items.
    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Add the project-specific path (e.g., `<project>/.claude/skills`).
    pub fn project_path(mut self, project_dir: &Path) -> Self {
        self.paths.push(
            project_dir
                .join(".claude")
                .join(self.strategy.config_subdir()),
        );
        self
    }

    /// Add the user-specific path (e.g., `~/.claude/skills`).
    pub fn user_path(mut self) -> Self {
        if let Some(home) = super::home_dir() {
            self.paths
                .push(home.join(".claude").join(self.strategy.config_subdir()));
        }
        self.source_type = SourceType::User;
        self
    }

    /// Set the priority of this provider.
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the source type for items loaded by this provider.
    pub fn source_type(mut self, source_type: SourceType) -> Self {
        self.source_type = source_type;
        self
    }

    /// Get the configured paths.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Get the loader reference.
    pub fn loader(&self) -> &L {
        &self.loader
    }
}

impl<T, L, S> Default for FileProvider<T, L, S>
where
    T: Named + Clone + Send + Sync,
    L: DocumentLoader<T> + Default,
    S: LookupStrategy + Default,
{
    fn default() -> Self {
        Self::new(L::default(), S::default())
    }
}

#[async_trait]
impl<T, L, S> Provider<T> for FileProvider<T, L, S>
where
    T: Named + Clone + Send + Sync + 'static,
    L: DocumentLoader<T> + 'static,
    S: LookupStrategy + 'static,
{
    fn provider_name(&self) -> &str {
        "file"
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn source_type(&self) -> SourceType {
        self.source_type
    }

    async fn list(&self) -> crate::Result<Vec<String>> {
        let items = self.load_all().await?;
        Ok(items
            .into_iter()
            .map(|item| item.name().to_string())
            .collect())
    }

    async fn get(&self, name: &str) -> crate::Result<Option<T>> {
        for path in &self.paths {
            if !path.exists() {
                continue;
            }

            if let Some(file_path) = self.strategy.find_by_name(path, name) {
                return Ok(Some(self.loader.load_file(&file_path).await?));
            }
        }
        Ok(None)
    }

    async fn load_all(&self) -> crate::Result<Vec<T>> {
        let mut items = Vec::new();

        for path in &self.paths {
            if path.exists() {
                let loaded = self.loader.load_directory(path).await?;
                items.extend(loaded);
            }
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestItem {
        name: String,
        content: String,
    }

    impl Named for TestItem {
        fn name(&self) -> &str {
            &self.name
        }
    }

    fn is_test_markdown(path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "md")
    }

    #[derive(Clone, Default)]
    struct TestLoader;

    impl DocumentLoader<TestItem> for TestLoader {
        fn parse_content(&self, content: &str, path: Option<&Path>) -> crate::Result<TestItem> {
            let name = path
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            Ok(TestItem {
                name,
                content: content.to_string(),
            })
        }

        fn doc_type_name(&self) -> &'static str {
            "test"
        }

        fn file_filter(&self) -> fn(&Path) -> bool {
            is_test_markdown
        }
    }

    #[derive(Clone, Default)]
    struct TestLookupStrategy;

    impl LookupStrategy for TestLookupStrategy {
        fn config_subdir(&self) -> &'static str {
            "test"
        }

        fn find_by_name(&self, dir: &Path, name: &str) -> Option<PathBuf> {
            let file = dir.join(format!("{}.md", name));
            if file.exists() { Some(file) } else { None }
        }
    }

    #[tokio::test]
    async fn test_file_provider_empty() {
        let provider: FileProvider<TestItem, TestLoader, TestLookupStrategy> =
            FileProvider::default();

        let items = provider.load_all().await.unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_file_provider_with_temp_dir() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("test.md");
        tokio::fs::write(&file, "test content").await.unwrap();

        let provider: FileProvider<TestItem, TestLoader, TestLookupStrategy> =
            FileProvider::new(TestLoader, TestLookupStrategy).path(temp.path());

        let items = provider.load_all().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "test");
        assert_eq!(items[0].content, "test content");
    }

    #[tokio::test]
    async fn test_file_provider_get_by_name() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("myitem.md");
        tokio::fs::write(&file, "my content").await.unwrap();

        let provider: FileProvider<TestItem, TestLoader, TestLookupStrategy> =
            FileProvider::new(TestLoader, TestLookupStrategy).path(temp.path());

        let item = provider.get("myitem").await.unwrap();
        assert!(item.is_some());
        assert_eq!(item.unwrap().name, "myitem");

        let missing = provider.get("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_file_provider_priority_and_source() {
        let provider: FileProvider<TestItem, TestLoader, TestLookupStrategy> =
            FileProvider::default()
                .priority(42)
                .source_type(SourceType::Builtin);

        assert_eq!(Provider::priority(&provider), 42);
        assert_eq!(Provider::source_type(&provider), SourceType::Builtin);
        assert_eq!(provider.provider_name(), "file");
    }
}
