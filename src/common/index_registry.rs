//! Index registry for managing collections of index entries.
//!
//! `IndexRegistry` provides a generic container for index entries with:
//! - Priority-based override semantics
//! - Cached content loading
//! - Summary generation for system prompts
//! - Optional path-matching for `PathMatched` types

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::{Index, PathMatched};

/// Generic registry for index entries.
///
/// Provides:
/// - Named lookup with priority-based overrides
/// - Lazy content loading with caching
/// - Summary generation for system prompts
///
/// # Example
///
/// ```ignore
/// let registry = IndexRegistry::new();
/// registry.register(skill_index);
///
/// // Get metadata (always fast)
/// let idx = registry.get("commit").unwrap();
///
/// // Load full content (lazy, cached)
/// let content = registry.load_content("commit").await?;
///
/// // Generate summary for system prompt
/// let summary = registry.build_summary();
/// ```
pub struct IndexRegistry<I: Index> {
    indices: HashMap<String, I>,
    content_cache: Arc<RwLock<HashMap<String, String>>>,
}

impl<I: Index> IndexRegistry<I> {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            indices: HashMap::new(),
            content_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an index entry.
    ///
    /// If an entry with the same name exists, it's replaced only if the
    /// new entry has equal or higher priority.
    pub fn register(&mut self, index: I) {
        let name = index.name().to_string();

        if let Some(existing) = self.indices.get(&name) {
            if index.priority() >= existing.priority() {
                self.indices.insert(name, index);
            }
        } else {
            self.indices.insert(name, index);
        }
    }

    /// Register multiple index entries.
    pub fn register_all(&mut self, indices: impl IntoIterator<Item = I>) {
        for index in indices {
            self.register(index);
        }
    }

    /// Get an index entry by name.
    pub fn get(&self, name: &str) -> Option<&I> {
        self.indices.get(name)
    }

    /// List all registered names.
    pub fn list(&self) -> Vec<&str> {
        self.indices.keys().map(String::as_str).collect()
    }

    /// Iterate over all index entries.
    pub fn iter(&self) -> impl Iterator<Item = &I> {
        self.indices.values()
    }

    /// Get the number of registered entries.
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Check if an entry with the given name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.indices.contains_key(name)
    }

    /// Remove an entry by name.
    ///
    /// Also clears the cached content for this entry.
    pub async fn remove(&mut self, name: &str) -> Option<I> {
        self.content_cache.write().await.remove(name);
        self.indices.remove(name)
    }

    /// Clear all entries.
    ///
    /// Also clears all cached content.
    pub async fn clear(&mut self) {
        self.indices.clear();
        self.content_cache.write().await.clear();
    }

    /// Load content for an index entry with caching.
    ///
    /// Returns cached content if available, otherwise loads from source.
    pub async fn load_content(&self, name: &str) -> crate::Result<String> {
        {
            let cache = self.content_cache.read().await;
            if let Some(content) = cache.get(name) {
                return Ok(content.clone());
            }
        }

        let index = self
            .indices
            .get(name)
            .ok_or_else(|| crate::Error::Config(format!("Index entry '{}' not found", name)))?;

        let content = index.load_content().await?;

        {
            let mut cache = self.content_cache.write().await;
            cache.insert(name.to_string(), content.clone());
        }

        Ok(content)
    }

    /// Invalidate cached content for an entry.
    pub async fn invalidate_cache(&self, name: &str) {
        let mut cache = self.content_cache.write().await;
        cache.remove(name);
    }

    /// Clear all cached content.
    pub async fn clear_cache(&self) {
        let mut cache = self.content_cache.write().await;
        cache.clear();
    }

    /// Build a summary of all entries for system prompt injection.
    ///
    /// Returns a formatted string with one summary line per entry.
    pub fn build_summary(&self) -> String {
        let mut lines: Vec<_> = self
            .indices
            .values()
            .map(|idx| idx.to_summary_line())
            .collect();
        lines.sort();
        lines.join("\n")
    }

    /// Build a summary with entries ordered by priority (highest first).
    ///
    /// Returns a formatted string with one summary line per entry,
    /// sorted by the entry's priority value.
    pub fn build_priority_summary(&self) -> String {
        self.sorted_by_priority()
            .iter()
            .map(|idx| idx.to_summary_line())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Build a summary with a custom formatter.
    pub fn build_summary_with<F>(&self, formatter: F) -> String
    where
        F: Fn(&I) -> String,
    {
        let mut lines: Vec<_> = self.indices.values().map(formatter).collect();
        lines.sort();
        lines.join("\n")
    }

    /// Get entries sorted by priority (highest first).
    pub fn sorted_by_priority(&self) -> Vec<&I> {
        let mut items: Vec<_> = self.indices.values().collect();
        items.sort_by_key(|i| std::cmp::Reverse(i.priority()));
        items
    }

    /// Filter entries by a predicate.
    pub fn filter<F>(&self, predicate: F) -> Vec<&I>
    where
        F: Fn(&I) -> bool,
    {
        self.indices.values().filter(|i| predicate(i)).collect()
    }
}

// ============================================================================
// PathMatched support
// ============================================================================

/// Loaded entry with index metadata and content.
#[derive(Clone, Debug)]
pub struct LoadedEntry<I: Index> {
    /// The index metadata.
    pub index: I,
    /// The loaded content.
    pub content: String,
}

impl<I: Index + PathMatched> IndexRegistry<I> {
    /// Find all entries that match the given file path.
    ///
    /// Returns entries sorted by priority (highest first).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let matching = registry.find_matching(Path::new("src/lib.rs"));
    /// for entry in matching {
    ///     println!("Matched: {}", entry.name());
    /// }
    /// ```
    pub fn find_matching(&self, path: &Path) -> Vec<&I> {
        let mut matches: Vec<_> = self
            .indices
            .values()
            .filter(|i| i.matches_path(path))
            .collect();
        // Sort by priority (highest first)
        matches.sort_by_key(|i| std::cmp::Reverse(i.priority()));
        matches
    }

    /// Load content for all entries matching the given file path.
    ///
    /// Returns loaded entries sorted by priority (highest first).
    /// Entries that fail to load are skipped.
    pub async fn load_matching(&self, path: &Path) -> Vec<LoadedEntry<I>> {
        let matching = self.find_matching(path);
        let mut results = Vec::with_capacity(matching.len());

        for index in matching {
            let name = index.name();
            match self.load_content(name).await {
                Ok(content) => {
                    results.push(LoadedEntry {
                        index: index.clone(),
                        content,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to load content for '{}': {}", name, e);
                }
            }
        }

        results
    }

    /// Check if any entry matches the given file path.
    pub fn has_matching(&self, path: &Path) -> bool {
        self.indices.values().any(|i| i.matches_path(path))
    }

    /// Build summary for entries matching a specific path.
    pub fn build_matching_summary(&self, path: &Path) -> String {
        let matching = self.find_matching(path);
        if matching.is_empty() {
            return String::new();
        }

        matching
            .into_iter()
            .map(|i| i.to_summary_line())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl<I: Index> Default for IndexRegistry<I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Index> Clone for IndexRegistry<I> {
    fn clone(&self) -> Self {
        Self {
            indices: self.indices.clone(),
            content_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl<I: Index> FromIterator<I> for IndexRegistry<I> {
    fn from_iter<T: IntoIterator<Item = I>>(iter: T) -> Self {
        let mut registry = Self::new();
        registry.register_all(iter);
        registry
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::common::{ContentSource, Named, SourceType};

    #[derive(Clone, Debug)]
    struct TestIndex {
        name: String,
        desc: String,
        source: ContentSource,
        source_type: SourceType,
    }

    impl TestIndex {
        fn new(name: &str, desc: &str, source_type: SourceType) -> Self {
            Self {
                name: name.into(),
                desc: desc.into(),
                source: ContentSource::in_memory(format!("Content for {}", name)),
                source_type,
            }
        }
    }

    impl Named for TestIndex {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[async_trait]
    impl Index for TestIndex {
        fn source(&self) -> &ContentSource {
            &self.source
        }

        fn source_type(&self) -> SourceType {
            self.source_type
        }

        fn to_summary_line(&self) -> String {
            format!("- {}: {}", self.name, self.desc)
        }
    }

    #[test]
    fn test_basic_operations() {
        let mut registry = IndexRegistry::new();

        registry.register(TestIndex::new("a", "Desc A", SourceType::User));
        registry.register(TestIndex::new("b", "Desc B", SourceType::User));

        assert_eq!(registry.len(), 2);
        assert!(registry.contains("a"));
        assert!(registry.contains("b"));
        assert!(!registry.contains("c"));
    }

    #[test]
    fn test_priority_override() {
        let mut registry = IndexRegistry::new();

        // Register builtin first
        registry.register(TestIndex::new("test", "Builtin", SourceType::Builtin));
        assert_eq!(registry.get("test").unwrap().desc, "Builtin");

        // User should override builtin
        registry.register(TestIndex::new("test", "User", SourceType::User));
        assert_eq!(registry.get("test").unwrap().desc, "User");

        // Project should override user
        registry.register(TestIndex::new("test", "Project", SourceType::Project));
        assert_eq!(registry.get("test").unwrap().desc, "Project");

        // Builtin should NOT override project
        registry.register(TestIndex::new("test", "Builtin2", SourceType::Builtin));
        assert_eq!(registry.get("test").unwrap().desc, "Project");
    }

    #[tokio::test]
    async fn test_content_loading() {
        let mut registry = IndexRegistry::new();
        registry.register(TestIndex::new("test", "Desc", SourceType::User));

        let content = registry.load_content("test").await.unwrap();
        assert_eq!(content, "Content for test");
    }

    #[tokio::test]
    async fn test_content_caching() {
        let mut registry = IndexRegistry::new();
        registry.register(TestIndex::new("test", "Desc", SourceType::User));

        // First load
        let content1 = registry.load_content("test").await.unwrap();

        // Second load should use cache
        let content2 = registry.load_content("test").await.unwrap();

        assert_eq!(content1, content2);
    }

    #[test]
    fn test_build_summary() {
        let mut registry = IndexRegistry::new();
        registry.register(TestIndex::new("commit", "Create commits", SourceType::User));
        registry.register(TestIndex::new("review", "Review code", SourceType::User));

        let summary = registry.build_summary();
        assert!(summary.contains("- commit: Create commits"));
        assert!(summary.contains("- review: Review code"));
    }

    #[test]
    fn test_from_iterator() {
        let indices = vec![
            TestIndex::new("a", "A", SourceType::User),
            TestIndex::new("b", "B", SourceType::User),
        ];

        let registry: IndexRegistry<TestIndex> = indices.into_iter().collect();
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_filter() {
        let mut registry = IndexRegistry::new();
        registry.register(TestIndex::new("builtin1", "B1", SourceType::Builtin));
        registry.register(TestIndex::new("user1", "U1", SourceType::User));
        registry.register(TestIndex::new("project1", "P1", SourceType::Project));

        let users = registry.filter(|i| i.source_type() == SourceType::User);
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name(), "user1");
    }

    #[test]
    fn test_sorted_by_priority() {
        let mut registry = IndexRegistry::new();
        registry.register(TestIndex::new("builtin", "B", SourceType::Builtin));
        registry.register(TestIndex::new("user", "U", SourceType::User));
        registry.register(TestIndex::new("project", "P", SourceType::Project));

        let sorted = registry.sorted_by_priority();
        assert_eq!(sorted[0].name(), "project");
        assert_eq!(sorted[1].name(), "user");
        assert_eq!(sorted[2].name(), "builtin");
    }

    // ========================================================================
    // PathMatched tests
    // ========================================================================

    #[derive(Clone, Debug)]
    struct PathMatchedIndex {
        name: String,
        desc: String,
        patterns: Option<Vec<String>>,
        source: ContentSource,
        source_type: SourceType,
    }

    impl PathMatchedIndex {
        fn new(name: &str, patterns: Option<Vec<&str>>, source_type: SourceType) -> Self {
            Self {
                name: name.into(),
                desc: format!("Desc for {}", name),
                patterns: patterns.map(|p| p.into_iter().map(String::from).collect()),
                source: ContentSource::in_memory(format!("Content for {}", name)),
                source_type,
            }
        }
    }

    impl Named for PathMatchedIndex {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[async_trait]
    impl Index for PathMatchedIndex {
        fn source(&self) -> &ContentSource {
            &self.source
        }

        fn source_type(&self) -> SourceType {
            self.source_type
        }

        fn to_summary_line(&self) -> String {
            format!("- {}: {}", self.name, self.desc)
        }
    }

    impl PathMatched for PathMatchedIndex {
        fn path_patterns(&self) -> Option<&[String]> {
            self.patterns.as_deref()
        }

        fn matches_path(&self, path: &Path) -> bool {
            match &self.patterns {
                None => true, // Global
                Some(patterns) if patterns.is_empty() => false,
                Some(patterns) => {
                    let path_str = path.to_string_lossy();
                    patterns.iter().any(|p| {
                        glob::Pattern::new(p)
                            .map(|pat| pat.matches(&path_str))
                            .unwrap_or(false)
                    })
                }
            }
        }
    }

    #[test]
    fn test_find_matching_global() {
        let mut registry = IndexRegistry::new();
        registry.register(PathMatchedIndex::new("global", None, SourceType::User));

        let matches = registry.find_matching(Path::new("any/file.rs"));
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name(), "global");
    }

    #[test]
    fn test_find_matching_with_patterns() {
        let mut registry = IndexRegistry::new();
        registry.register(PathMatchedIndex::new(
            "rust",
            Some(vec!["**/*.rs"]),
            SourceType::User,
        ));
        registry.register(PathMatchedIndex::new(
            "typescript",
            Some(vec!["**/*.ts", "**/*.tsx"]),
            SourceType::User,
        ));

        let rust_matches = registry.find_matching(Path::new("src/lib.rs"));
        assert_eq!(rust_matches.len(), 1);
        assert_eq!(rust_matches[0].name(), "rust");

        let ts_matches = registry.find_matching(Path::new("src/app.tsx"));
        assert_eq!(ts_matches.len(), 1);
        assert_eq!(ts_matches[0].name(), "typescript");
    }

    #[test]
    fn test_find_matching_sorted_by_priority() {
        let mut registry = IndexRegistry::new();
        registry.register(PathMatchedIndex::new("builtin", None, SourceType::Builtin));
        registry.register(PathMatchedIndex::new("user", None, SourceType::User));
        registry.register(PathMatchedIndex::new("project", None, SourceType::Project));

        let matches = registry.find_matching(Path::new("any/file.rs"));
        assert_eq!(matches.len(), 3);
        // Should be sorted by priority (highest first)
        assert_eq!(matches[0].name(), "project");
        assert_eq!(matches[1].name(), "user");
        assert_eq!(matches[2].name(), "builtin");
    }

    #[test]
    fn test_has_matching() {
        let mut registry = IndexRegistry::new();
        registry.register(PathMatchedIndex::new(
            "rust",
            Some(vec!["**/*.rs"]),
            SourceType::User,
        ));

        assert!(registry.has_matching(Path::new("src/lib.rs")));
        assert!(!registry.has_matching(Path::new("src/lib.ts")));
    }

    #[tokio::test]
    async fn test_load_matching() {
        let mut registry = IndexRegistry::new();
        registry.register(PathMatchedIndex::new(
            "rust",
            Some(vec!["**/*.rs"]),
            SourceType::User,
        ));
        registry.register(PathMatchedIndex::new("global", None, SourceType::User));

        let loaded = registry.load_matching(Path::new("src/lib.rs")).await;
        assert_eq!(loaded.len(), 2);

        // Both should have loaded content
        for entry in &loaded {
            assert!(entry.content.starts_with("Content for"));
        }
    }
}
