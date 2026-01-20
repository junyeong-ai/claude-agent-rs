//! CLI-compatible @import extraction using Markdown-aware parsing.
//!
//! This module provides CLI-compatible import path extraction from Markdown files.
//! It skips code blocks (fenced and inline) to avoid extracting @paths from code examples.

use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Extracts @import paths from Markdown content, CLI-compatible implementation.
///
/// # CLI Compatibility
/// This implementation matches the Claude Code CLI 2.1.12 behavior:
/// - Uses regex pattern: `(?:^|\s)@((?:[^\s\\]|\\ )+)` to match @paths
/// - Skips content inside fenced code blocks (```)
/// - Skips content inside inline code spans (backticks)
/// - Supports escaped spaces in paths (`\ `)
/// - Validates paths using the same rules as CLI
pub struct ImportExtractor {
    regex: Regex,
    code_block_regex: Regex,
    inline_code_regex: Regex,
}

impl ImportExtractor {
    /// Creates a new ImportExtractor with CLI-compatible regex.
    pub fn new() -> Self {
        Self {
            // CLI-compatible regex: matches @path at line start or after whitespace
            // Captures: path with escaped spaces (\ )
            regex: Regex::new(r"(?:^|\s)@((?:[^\s\\]|\\ )+)").expect("Invalid regex pattern"),
            // Match fenced code blocks (``` or ~~~)
            code_block_regex: Regex::new(r"(?s)(?:```|~~~).*?(?:```|~~~)").expect("Invalid regex"),
            // Match inline code (`...`)
            inline_code_regex: Regex::new(r"`[^`]+`").expect("Invalid regex"),
        }
    }

    /// Extracts @import paths from Markdown content, skipping code blocks.
    ///
    /// # Arguments
    /// * `content` - Markdown content to parse
    /// * `base_dir` - Base directory for resolving relative paths
    ///
    /// # Returns
    /// Vector of unique resolved PathBufs for valid import paths (duplicates removed)
    pub fn extract(&self, content: &str, base_dir: &Path) -> Vec<PathBuf> {
        // Remove fenced code blocks first
        let without_fenced = self.code_block_regex.replace_all(content, " ");
        // Remove inline code spans
        let clean_content = self.inline_code_regex.replace_all(&without_fenced, " ");

        // Extract paths from cleaned content with deduplication
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        self.extract_from_text_dedup(&clean_content, base_dir, &mut seen, &mut paths);
        paths
    }

    /// Extracts @import paths with deduplication.
    fn extract_from_text_dedup(
        &self,
        text: &str,
        base_dir: &Path,
        seen: &mut HashSet<PathBuf>,
        paths: &mut Vec<PathBuf>,
    ) {
        for cap in self.regex.captures_iter(text) {
            if let Some(m) = cap.get(1) {
                // Unescape spaces (CLI compatibility: `\ ` -> ` `)
                let raw_path = m.as_str().replace("\\ ", " ");
                if let Some(resolved) = self.resolve_path(&raw_path, base_dir) {
                    // Only add if not seen before
                    if seen.insert(resolved.clone()) {
                        paths.push(resolved);
                    }
                }
            }
        }
    }

    /// Resolves a path string to an absolute PathBuf.
    ///
    /// # Path Resolution Rules (CLI-compatible)
    /// - `~/...` -> Expands to home directory
    /// - `/...` -> Absolute path (as-is)
    /// - `./...` or relative -> Relative to base_dir
    fn resolve_path(&self, path: &str, base_dir: &Path) -> Option<PathBuf> {
        if !self.is_valid_path(path) {
            return None;
        }

        Some(if let Some(rest) = path.strip_prefix("~/") {
            crate::common::home_dir()?.join(rest)
        } else if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            base_dir.join(path)
        })
    }

    /// Validates a path string using CLI-compatible rules.
    ///
    /// # Valid Path Patterns (CLI: isValidPath)
    /// - Starts with `./` (explicit relative)
    /// - Starts with `~/` (home directory)
    /// - Starts with `/` but not just `/` alone (absolute path)
    /// - Starts with alphanumeric, `.`, `_`, or `-` (implicit relative)
    /// - Does NOT start with `@` (escaped @)
    /// - Does NOT start with special characters `#%^&*()`
    fn is_valid_path(&self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }

        // CLI-compatible validation rules
        path.starts_with("./")
            || path.starts_with("~/")
            || (path.starts_with('/') && path != "/")
            || (!path.starts_with('@')
                && !path.starts_with(|c| "#%^&*()".contains(c))
                && path
                    .starts_with(|c: char| c.is_alphanumeric() || c == '.' || c == '_' || c == '-'))
    }
}

impl Default for ImportExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_line_start() {
        let extractor = ImportExtractor::new();
        let content = "@docs/api.md\n@config/settings.md";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 2);
        assert!(imports[0].ends_with("docs/api.md"));
        assert!(imports[1].ends_with("config/settings.md"));
    }

    #[test]
    fn test_extract_inline() {
        let extractor = ImportExtractor::new();
        let content = "Prerequisites: @docs/guide.md for details";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 1);
        assert!(imports[0].ends_with("docs/guide.md"));
    }

    #[test]
    fn test_skip_fenced_code_block() {
        let extractor = ImportExtractor::new();
        let content = "```\n@should/not/import.md\n```\n@should/import.md";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 1);
        assert!(imports[0].ends_with("should/import.md"));
    }

    #[test]
    fn test_skip_indented_code_block() {
        let extractor = ImportExtractor::new();
        let content =
            "Normal text @real/import.md\n\n    @indented/code.md\n\nMore @another/import.md";
        let imports = extractor.extract(content, Path::new("/project"));
        // Note: Indented code blocks are NOT skipped (CLI-compatible behavior)
        // Only fenced code blocks (``` or ~~~) and inline code are skipped
        assert!(imports.iter().any(|p| p.ends_with("real/import.md")));
        assert!(imports.iter().any(|p| p.ends_with("another/import.md")));
    }

    #[test]
    fn test_skip_inline_code() {
        let extractor = ImportExtractor::new();
        let content = "Use `@decorator` syntax and @real/import.md file";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 1);
        assert!(imports[0].ends_with("real/import.md"));
    }

    #[test]
    fn test_home_expansion() {
        let extractor = ImportExtractor::new();
        let content = "@~/shared/config.md";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 1);
        assert!(!imports[0].to_string_lossy().contains('~'));
    }

    #[test]
    fn test_relative_paths() {
        let extractor = ImportExtractor::new();
        let content = "@./local/file.md";
        let imports = extractor.extract(content, Path::new("/project/subdir"));
        assert_eq!(imports.len(), 1);
        assert!(imports[0].starts_with("/project/subdir"));
    }

    #[test]
    fn test_absolute_path() {
        let extractor = ImportExtractor::new();
        let content = "@/absolute/path/file.md";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0], PathBuf::from("/absolute/path/file.md"));
    }

    #[test]
    fn test_invalid_paths_ignored() {
        let extractor = ImportExtractor::new();
        let content = "@#invalid @%also-invalid @^nope @&bad @*bad @(bad @)bad";
        let imports = extractor.extract(content, Path::new("/project"));
        assert!(imports.is_empty());
    }

    #[test]
    fn test_escaped_at_ignored() {
        let extractor = ImportExtractor::new();
        let content = "@@escaped @valid/path.md";
        let imports = extractor.extract(content, Path::new("/project"));
        // @@escaped should produce @escaped which starts with @ and is invalid
        // But @valid/path.md should be valid
        assert!(imports.iter().any(|p| p.ends_with("valid/path.md")));
    }

    #[test]
    fn test_escaped_spaces_in_path() {
        let extractor = ImportExtractor::new();
        let content = r"@docs/my\ file.md";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 1);
        assert!(imports[0].ends_with("docs/my file.md"));
    }

    #[test]
    fn test_root_slash_only_invalid() {
        let extractor = ImportExtractor::new();
        // "/" alone is not a valid import path
        assert!(!extractor.is_valid_path("/"));
    }

    #[test]
    fn test_implicit_relative_path() {
        let extractor = ImportExtractor::new();

        // Verify is_valid_path accepts each pattern
        assert!(
            extractor.is_valid_path("docs/file.md"),
            "alphanumeric start"
        );
        assert!(
            extractor.is_valid_path("_private/config.md"),
            "underscore start"
        );
        assert!(
            extractor.is_valid_path(".hidden/file.md"),
            "dot start (not ./)"
        );

        // Test extraction of each pattern individually
        let content1 = "@docs/file.md";
        let imports1 = extractor.extract(content1, Path::new("/project"));
        assert_eq!(imports1.len(), 1, "alphanumeric path");

        let content2 = "@_private/config.md";
        let imports2 = extractor.extract(content2, Path::new("/project"));
        assert_eq!(imports2.len(), 1, "underscore path");

        let content3 = "@.hidden/file.md";
        let imports3 = extractor.extract(content3, Path::new("/project"));
        assert_eq!(imports3.len(), 1, "dot path");
    }

    #[test]
    fn test_multiple_imports_same_line() {
        let extractor = ImportExtractor::new();
        let content = "Include @first.md and @second.md and @third.md";
        let imports = extractor.extract(content, Path::new("/project"));
        assert_eq!(imports.len(), 3);
    }

    #[test]
    fn test_empty_content() {
        let extractor = ImportExtractor::new();
        let imports = extractor.extract("", Path::new("/project"));
        assert!(imports.is_empty());
    }

    #[test]
    fn test_no_imports() {
        let extractor = ImportExtractor::new();
        let content = "# Title\n\nJust regular content without any imports.";
        let imports = extractor.extract(content, Path::new("/project"));
        assert!(imports.is_empty());
    }

    #[test]
    fn test_markdown_link_not_imported() {
        let extractor = ImportExtractor::new();
        // Markdown link format: [@text](@path) should NOT be extracted
        // because @ follows [ and ( which are not whitespace
        let content = "See [@.agents/docs.md](@.agents/docs.md) for details";
        let imports = extractor.extract(content, Path::new("/project"));
        assert!(
            imports.is_empty(),
            "Markdown links should not be extracted as imports"
        );
    }

    #[test]
    fn test_duplicate_import_paths_deduped() {
        let extractor = ImportExtractor::new();
        // Same file referenced twice on different lines
        let content = "@docs/api.md\nSome text\n@docs/api.md";
        let imports = extractor.extract(content, Path::new("/project"));
        println!("Extracted paths: {:?}", imports);
        // Now deduplicates - only 1 unique path
        assert_eq!(imports.len(), 1, "Duplicates should be removed");
        assert!(imports[0].ends_with("docs/api.md"));
    }

    #[test]
    fn test_same_file_inline_twice_deduped() {
        let extractor = ImportExtractor::new();
        // Same file mentioned twice on same line
        let content = "See @docs/api.md and also @docs/api.md";
        let imports = extractor.extract(content, Path::new("/project"));
        println!("Extracted paths: {:?}", imports);
        // Now deduplicates
        assert_eq!(imports.len(), 1, "Duplicates should be removed");
    }

    #[test]
    fn test_different_paths_not_deduped() {
        let extractor = ImportExtractor::new();
        // Different files should all be included
        let content = "@docs/api.md\n@docs/guide.md\n@docs/api.md";
        let imports = extractor.extract(content, Path::new("/project"));
        println!("Extracted paths: {:?}", imports);
        // Should have 2 unique paths (api.md and guide.md)
        assert_eq!(imports.len(), 2, "Different paths should be preserved");
    }
}
