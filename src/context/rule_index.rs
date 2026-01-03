//! Rule Index for Progressive Disclosure
//!
//! Rules are loaded on-demand based on file path matching.
//! Only indices (metadata) are loaded at startup; full content is lazy-loaded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleIndex {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    #[serde(default)]
    pub priority: i32,
    pub source: RuleSource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleSource {
    File { path: PathBuf },
    InMemory { content: String },
}

#[derive(Clone, Debug)]
pub struct LoadedRule {
    pub index: RuleIndex,
    pub content: String,
}

impl RuleIndex {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            paths: None,
            priority: 0,
            source: RuleSource::File {
                path: PathBuf::new(),
            },
        }
    }

    pub fn with_paths(mut self, paths: Vec<String>) -> Self {
        self.paths = Some(paths);
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_source(mut self, source: RuleSource) -> Self {
        self.source = source;
        self
    }

    pub fn from_file(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Self::parse_with_frontmatter(&content, path)
    }

    pub fn parse_with_frontmatter(content: &str, path: &Path) -> Option<Self> {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let (paths, priority) = Self::extract_frontmatter(content);

        Some(Self {
            name,
            paths,
            priority,
            source: RuleSource::File {
                path: path.to_path_buf(),
            },
        })
    }

    fn extract_frontmatter(content: &str) -> (Option<Vec<String>>, i32) {
        let trimmed = content.trim();
        if !trimmed.starts_with("---") {
            return (None, 0);
        }

        let rest = &trimmed[3..];
        let Some(end) = rest.find("---") else {
            return (None, 0);
        };
        let yaml = &rest[..end];

        let mut paths: Option<Vec<String>> = None;
        let mut priority = 0;

        for line in yaml.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("paths:") {
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                paths = Some(
                    value
                        .split(',')
                        .map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            } else if line.starts_with("- ") && paths.is_some() {
                let item = line[2..].trim().trim_matches(|c| c == '"' || c == '\'');
                if let Some(ref mut p) = paths
                    && !item.is_empty()
                {
                    p.push(item.to_string());
                }
            } else if let Some(val) = line.strip_prefix("priority:") {
                priority = val.trim().parse().unwrap_or(0);
            }
        }

        (paths, priority)
    }

    pub fn matches_path(&self, file_path: &Path) -> bool {
        match &self.paths {
            None => true,
            Some(patterns) => patterns.iter().any(|p| Self::glob_match(p, file_path)),
        }
    }

    fn glob_match(pattern: &str, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        glob::Pattern::new(pattern)
            .map(|p| p.matches(&path_str))
            .unwrap_or_else(|_| path_str.contains(pattern))
    }

    pub async fn load_content(&self) -> Option<String> {
        match &self.source {
            RuleSource::File { path } => {
                let content = tokio::fs::read_to_string(path).await.ok()?;
                Some(Self::strip_frontmatter(&content))
            }
            RuleSource::InMemory { content } => Some(content.clone()),
        }
    }

    fn strip_frontmatter(content: &str) -> String {
        let trimmed = content.trim();
        if !trimmed.starts_with("---") {
            return content.to_string();
        }

        let rest = &trimmed[3..];
        if let Some(end) = rest.find("---") {
            rest[end + 3..].trim().to_string()
        } else {
            content.to_string()
        }
    }
}

pub struct RulesEngine {
    indices: Vec<RuleIndex>,
    cache: RwLock<HashMap<String, LoadedRule>>,
}

impl Default for RulesEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RulesEngine {
    pub fn new() -> Self {
        Self {
            indices: Vec::new(),
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_index(&mut self, index: RuleIndex) {
        self.indices.push(index);
        self.indices.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn add_indices(&mut self, indices: impl IntoIterator<Item = RuleIndex>) {
        for index in indices {
            self.indices.push(index);
        }
        self.indices.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn indices(&self) -> &[RuleIndex] {
        &self.indices
    }

    pub fn find_matching(&self, file_path: &Path) -> Vec<&RuleIndex> {
        self.indices
            .iter()
            .filter(|r| r.matches_path(file_path))
            .collect()
    }

    pub async fn load_matching(&self, file_path: &Path) -> Vec<LoadedRule> {
        let matching = self.find_matching(file_path);
        let mut results = Vec::with_capacity(matching.len());

        for index in matching {
            if let Some(rule) = self.load_rule(&index.name).await {
                results.push(rule);
            }
        }

        results
    }

    pub async fn load_rule(&self, name: &str) -> Option<LoadedRule> {
        {
            let cache = self.cache.read().await;
            if let Some(rule) = cache.get(name) {
                return Some(rule.clone());
            }
        }

        let index = self.indices.iter().find(|i| i.name == name)?;
        let content = index.load_content().await?;

        let rule = LoadedRule {
            index: index.clone(),
            content,
        };

        {
            let mut cache = self.cache.write().await;
            cache.insert(name.to_string(), rule.clone());
        }

        Some(rule)
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    pub fn build_summary(&self) -> String {
        if self.indices.is_empty() {
            return String::new();
        }

        let mut lines = vec!["# Available Rules".to_string()];
        for rule in &self.indices {
            let scope = match &rule.paths {
                Some(p) => p.join(", "),
                None => "all files".to_string(),
            };
            lines.push(format!("- {}: applies to {}", rule.name, scope));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;

    #[test]
    fn test_rule_index_creation() {
        let rule = RuleIndex::new("typescript")
            .with_paths(vec!["**/*.ts".into(), "**/*.tsx".into()])
            .with_priority(10);

        assert_eq!(rule.name, "typescript");
        assert_eq!(rule.priority, 10);
    }

    #[test]
    fn test_path_matching() {
        let rule = RuleIndex::new("rust").with_paths(vec!["**/*.rs".into()]);

        assert!(rule.matches_path(Path::new("src/lib.rs")));
        assert!(rule.matches_path(Path::new("src/context/mod.rs")));
        assert!(!rule.matches_path(Path::new("src/lib.ts")));
    }

    #[test]
    fn test_global_rule() {
        let rule = RuleIndex::new("security");
        assert!(rule.matches_path(Path::new("any/file.rs")));
        assert!(rule.matches_path(Path::new("another/file.js")));
    }

    #[test]
    fn test_frontmatter_parsing() {
        let content = r#"---
paths: src/**/*.rs, tests/**/*.rs
priority: 10
---

# Rust Guidelines
Use snake_case for variables."#;

        let (paths, priority) = RuleIndex::extract_frontmatter(content);
        assert_eq!(priority, 10);
        assert!(paths.is_some());
        let paths = paths.unwrap();
        assert!(paths.contains(&"src/**/*.rs".to_string()));
        assert!(paths.contains(&"tests/**/*.rs".to_string()));
    }

    #[test]
    fn test_strip_frontmatter() {
        let content = r#"---
paths: src/**/*.rs
---

# Content"#;

        let stripped = RuleIndex::strip_frontmatter(content);
        assert_eq!(stripped, "# Content");
    }

    #[tokio::test]
    async fn test_rules_engine_matching() {
        let mut engine = RulesEngine::new();

        engine.add_index(RuleIndex::new("rust").with_paths(vec!["**/*.rs".into()]));
        engine.add_index(RuleIndex::new("global"));

        let matches = engine.find_matching(Path::new("src/lib.rs"));
        assert_eq!(matches.len(), 2);
    }

    #[tokio::test]
    async fn test_lazy_loading() {
        let dir = tempdir().unwrap();
        let rule_path = dir.path().join("test.md");
        fs::write(
            &rule_path,
            r#"---
paths: **/*.rs
priority: 5
---

# Test Rule Content"#,
        )
        .await
        .unwrap();

        let index = RuleIndex::from_file(&rule_path).unwrap();
        assert_eq!(index.name, "test");
        assert_eq!(index.priority, 5);

        let content = index.load_content().await.unwrap();
        assert_eq!(content, "# Test Rule Content");
    }
}
