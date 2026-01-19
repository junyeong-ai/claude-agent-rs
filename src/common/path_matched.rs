//! Path-matching trait for context-sensitive index entries.
//!
//! The `PathMatched` trait enables index entries to be filtered based on file paths.
//! This is essential for rules that apply only to specific file patterns.
//!
//! # Example
//!
//! ```ignore
//! pub struct RuleIndex {
//!     paths: Option<Vec<String>>,
//!     compiled_patterns: Vec<Pattern>,
//!     // ...
//! }
//!
//! impl PathMatched for RuleIndex {
//!     fn path_patterns(&self) -> Option<&[String]> {
//!         self.paths.as_deref()
//!     }
//!
//!     fn matches_path(&self, path: &Path) -> bool {
//!         if self.compiled_patterns.is_empty() {
//!             return true; // Global rule matches all
//!         }
//!         let path_str = path.to_string_lossy();
//!         self.compiled_patterns.iter().any(|p| p.matches(&path_str))
//!     }
//! }
//! ```

use std::path::Path;

/// Trait for index entries that support path-based filtering.
///
/// Implementors can specify glob patterns that determine which files
/// the entry applies to. Entries without patterns match all files.
pub trait PathMatched {
    /// Get the path patterns this entry matches.
    ///
    /// Returns `None` if this is a global entry that matches all files.
    /// Returns `Some(&[])` if patterns were explicitly set to empty (matches nothing).
    fn path_patterns(&self) -> Option<&[String]>;

    /// Check if this entry matches the given file path.
    ///
    /// Default behavior:
    /// - No patterns (`None`) → matches all files
    /// - Empty patterns → matches no files
    /// - Has patterns → matches if any pattern matches
    fn matches_path(&self, path: &Path) -> bool;

    /// Check if this is a global entry (matches all files).
    fn is_global(&self) -> bool {
        self.path_patterns().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPathMatched {
        patterns: Option<Vec<String>>,
    }

    impl PathMatched for TestPathMatched {
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
    fn test_global_matches_all() {
        let global = TestPathMatched { patterns: None };
        assert!(global.is_global());
        assert!(global.matches_path(Path::new("any/file.rs")));
        assert!(global.matches_path(Path::new("other/path.ts")));
    }

    #[test]
    fn test_pattern_matching() {
        let rust_only = TestPathMatched {
            patterns: Some(vec!["**/*.rs".to_string()]),
        };
        assert!(!rust_only.is_global());
        assert!(rust_only.matches_path(Path::new("src/lib.rs")));
        assert!(rust_only.matches_path(Path::new("tests/integration.rs")));
        assert!(!rust_only.matches_path(Path::new("src/lib.ts")));
    }

    #[test]
    fn test_multiple_patterns() {
        let web = TestPathMatched {
            patterns: Some(vec!["**/*.ts".to_string(), "**/*.tsx".to_string()]),
        };
        assert!(web.matches_path(Path::new("src/app.ts")));
        assert!(web.matches_path(Path::new("components/Button.tsx")));
        assert!(!web.matches_path(Path::new("src/lib.rs")));
    }

    #[test]
    fn test_empty_patterns_matches_nothing() {
        let empty = TestPathMatched {
            patterns: Some(vec![]),
        };
        assert!(!empty.is_global());
        assert!(!empty.matches_path(Path::new("any/file.rs")));
    }
}
