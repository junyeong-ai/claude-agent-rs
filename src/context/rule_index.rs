//! Rule Index Types for Progressive Disclosure
//!
//! RuleIndex contains path patterns for conditional rule loading.
//! Rules are only loaded when the current working file matches their path patterns.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Rule index entry - contains conditions for rule activation
///
/// Rules are loaded on-demand when the current file path matches
/// the rule's path patterns.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleIndex {
    /// Rule name (usually the filename without extension)
    pub name: String,

    /// Path patterns (glob) that trigger this rule
    ///
    /// If None, the rule applies to all files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,

    /// Priority (higher values have precedence)
    #[serde(default)]
    pub priority: i32,

    /// Source location for loading the full rule
    pub source: RuleSource,
}

/// Source location for loading the full rule content
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleSource {
    /// File system path (.claude/rules/)
    File {
        /// Path to the rule markdown file
        path: PathBuf,
    },

    /// Database storage (server environment)
    Database {
        /// Rule ID in database
        rule_id: String,
    },
}

/// A loaded rule with its full content
#[derive(Clone, Debug)]
pub struct LoadedRule {
    /// Rule index metadata
    pub index: RuleIndex,

    /// Full rule content (markdown)
    pub content: String,

    /// When the rule was loaded
    pub loaded_at: DateTime<Utc>,
}

impl RuleIndex {
    /// Create a new rule index entry
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

    /// Set path patterns
    pub fn with_paths(mut self, paths: Vec<String>) -> Self {
        self.paths = Some(paths);
        self
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set source location
    pub fn with_source(mut self, source: RuleSource) -> Self {
        self.source = source;
        self
    }

    /// Check if this rule applies to the given file path
    ///
    /// If `paths` is None, the rule applies to all files.
    /// Otherwise, at least one pattern must match.
    pub fn matches_path(&self, file_path: &Path) -> bool {
        match &self.paths {
            None => true, // No path restriction = applies to all
            Some(patterns) => patterns
                .iter()
                .any(|pattern| Self::glob_match(pattern, file_path)),
        }
    }

    /// Simple glob matching for path patterns
    ///
    /// Supports:
    /// - `*` matches any characters except `/`
    /// - `**` matches any characters including `/`
    /// - `?` matches any single character
    fn glob_match(pattern: &str, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Use the glob crate for pattern matching
        match glob::Pattern::new(pattern) {
            Ok(pat) => pat.matches(&path_str),
            Err(_) => {
                // Fall back to simple contains check
                path_str.contains(pattern)
            }
        }
    }
}

impl LoadedRule {
    /// Create a new loaded rule
    pub fn new(index: RuleIndex, content: String) -> Self {
        Self {
            index,
            content,
            loaded_at: Utc::now(),
        }
    }

    /// Check if the rule should be refreshed based on age
    pub fn is_stale(&self, max_age: chrono::Duration) -> bool {
        Utc::now() - self.loaded_at > max_age
    }
}

/// Rules engine for evaluating and loading rules
#[derive(Debug, Default)]
pub struct RulesEngine {
    /// Rule indices (always loaded)
    indices: Vec<RuleIndex>,

    /// Loaded rule cache
    loaded: std::collections::HashMap<String, LoadedRule>,
}

impl RulesEngine {
    /// Create a new rules engine
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule index
    pub fn add_index(&mut self, index: RuleIndex) {
        self.indices.push(index);
        // Sort by priority (descending)
        self.indices.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Get all rule indices
    pub fn indices(&self) -> &[RuleIndex] {
        &self.indices
    }

    /// Find rules that apply to the given file path
    pub fn find_matching(&self, file_path: &Path) -> Vec<&RuleIndex> {
        self.indices
            .iter()
            .filter(|r| r.matches_path(file_path))
            .collect()
    }

    /// Cache a loaded rule
    pub fn cache_rule(&mut self, rule: LoadedRule) {
        self.loaded.insert(rule.index.name.clone(), rule);
    }

    /// Get a cached rule by name
    pub fn get_cached(&self, name: &str) -> Option<&LoadedRule> {
        self.loaded.get(name)
    }

    /// Clear the loaded rule cache
    pub fn clear_cache(&mut self) {
        self.loaded.clear();
    }

    /// Generate a summary of all rule indices
    pub fn summary(&self) -> String {
        if self.indices.is_empty() {
            return "No rules defined.".to_string();
        }

        let mut lines = vec!["Available rules:".to_string()];
        for rule in &self.indices {
            let paths = match &rule.paths {
                Some(p) => p.join(", "),
                None => "all files".to_string(),
            };
            lines.push(format!("- {}: applies to {}", rule.name, paths));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // No paths = applies to all
        assert!(rule.matches_path(Path::new("any/file.rs")));
        assert!(rule.matches_path(Path::new("another/file.js")));
    }

    #[test]
    fn test_rules_engine() {
        let mut engine = RulesEngine::new();

        engine.add_index(RuleIndex::new("low").with_priority(0));
        engine.add_index(RuleIndex::new("high").with_priority(10));
        engine.add_index(RuleIndex::new("medium").with_priority(5));

        // Should be sorted by priority
        let indices = engine.indices();
        assert_eq!(indices[0].name, "high");
        assert_eq!(indices[1].name, "medium");
        assert_eq!(indices[2].name, "low");
    }

    #[test]
    fn test_find_matching_rules() {
        let mut engine = RulesEngine::new();

        engine.add_index(RuleIndex::new("rust").with_paths(vec!["**/*.rs".into()]));
        engine.add_index(RuleIndex::new("global")); // No paths = matches all

        let matches = engine.find_matching(Path::new("src/lib.rs"));
        assert_eq!(matches.len(), 2);
    }
}
