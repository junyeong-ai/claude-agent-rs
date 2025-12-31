//! CLAUDE.md memory file loader with recursive loading and import support.
//!
//! Implements Claude Code CLI compatible memory loading:
//! - Recursive loading from current directory to root
//! - CLAUDE.local.md support (auto-gitignored)
//! - @import syntax for file inclusion
//! - .claude/rules/ directory loading

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{ContextError, ContextResult};

/// Memory loader for CLAUDE.md files.
#[derive(Debug, Default)]
pub struct MemoryLoader {
    /// Loaded file paths (to prevent circular imports)
    loaded_paths: HashSet<PathBuf>,
}

impl MemoryLoader {
    /// Create a new memory loader.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load all memory content starting from a directory.
    ///
    /// Loads in order:
    /// 1. CLAUDE.md files recursively from current to root
    /// 2. CLAUDE.local.md files (not committed to git)
    /// 3. .claude/rules/*.md files
    pub async fn load_all(&mut self, start_dir: &Path) -> ContextResult<MemoryContent> {
        let mut content = MemoryContent::default();

        // 1. Load CLAUDE.md files recursively (from root to current for proper ordering)
        let claude_files = self.find_claude_files(start_dir);
        for path in claude_files {
            if let Ok(text) = self.load_file_with_imports(&path).await {
                content.claude_md.push(text);
            }
        }

        // 2. Load CLAUDE.local.md files
        let local_files = self.find_local_files(start_dir);
        for path in local_files {
            if let Ok(text) = self.load_file_with_imports(&path).await {
                content.local_md.push(text);
            }
        }

        // 3. Load .claude/rules/ directory
        let rules_dir = start_dir.join(".claude").join("rules");
        if rules_dir.exists() {
            let rules = self.load_rules_directory(&rules_dir).await?;
            content.rules = rules;
        }

        Ok(content)
    }

    /// Find all CLAUDE.md files from start_dir to root.
    fn find_claude_files(&self, start_dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut current = start_dir.to_path_buf();

        loop {
            // Check ./CLAUDE.md
            let claude_md = current.join("CLAUDE.md");
            if claude_md.exists() {
                files.push(claude_md);
            }

            // Check ./.claude/CLAUDE.md
            let claude_dir_md = current.join(".claude").join("CLAUDE.md");
            if claude_dir_md.exists() {
                files.push(claude_dir_md);
            }

            // Move to parent
            if let Some(parent) = current.parent() {
                if parent == current || parent.as_os_str().is_empty() {
                    break;
                }
                current = parent.to_path_buf();
            } else {
                break;
            }
        }

        // Reverse to load from root to current (more general to more specific)
        files.reverse();
        files
    }

    /// Find all CLAUDE.local.md files from start_dir to root.
    fn find_local_files(&self, start_dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut current = start_dir.to_path_buf();

        loop {
            let local_md = current.join("CLAUDE.local.md");
            if local_md.exists() {
                files.push(local_md);
            }

            let local_dir_md = current.join(".claude").join("CLAUDE.local.md");
            if local_dir_md.exists() {
                files.push(local_dir_md);
            }

            if let Some(parent) = current.parent() {
                if parent == current || parent.as_os_str().is_empty() {
                    break;
                }
                current = parent.to_path_buf();
            } else {
                break;
            }
        }

        files.reverse();
        files
    }

    /// Load a file and process @import directives.
    fn load_file_with_imports<'a>(
        &'a mut self,
        path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ContextResult<String>> + Send + 'a>>
    {
        Box::pin(async move {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

            // Prevent circular imports
            if self.loaded_paths.contains(&canonical) {
                return Ok(String::new());
            }
            self.loaded_paths.insert(canonical.clone());

            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| ContextError::Source {
                    message: format!("Failed to read {}: {}", path.display(), e),
                })?;

            // Process @import directives
            self.process_imports(&content, path.parent().unwrap_or(Path::new(".")))
                .await
        })
    }

    /// Expand home directory (~) in path.
    fn expand_home(path: &str) -> PathBuf {
        if let Some(rest) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(rest);
            }
        }
        PathBuf::from(path)
    }

    /// Process @import directives in content.
    fn process_imports<'a>(
        &'a mut self,
        content: &'a str,
        base_dir: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ContextResult<String>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut result = String::new();

            for line in content.lines() {
                let trimmed = line.trim();

                if trimmed.starts_with('@') && !trimmed.starts_with("@@") {
                    let import_path = trimmed.trim_start_matches('@').trim();
                    if !import_path.is_empty() {
                        let full_path = if import_path.starts_with("~/") {
                            Self::expand_home(import_path)
                        } else if import_path.starts_with('/') {
                            PathBuf::from(import_path)
                        } else {
                            base_dir.join(import_path)
                        };

                        if full_path.exists() {
                            match self.load_file_with_imports(&full_path).await {
                                Ok(imported) => {
                                    result.push_str(&imported);
                                    result.push('\n');
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to import {}: {}", import_path, e);
                                    result.push_str(line);
                                    result.push('\n');
                                }
                            }
                        } else {
                            result.push_str(line);
                            result.push('\n');
                        }
                    } else {
                        result.push_str(line);
                        result.push('\n');
                    }
                } else {
                    result.push_str(line);
                    result.push('\n');
                }
            }

            Ok(result)
        })
    }

    /// Load all .md files from a rules directory.
    async fn load_rules_directory(&mut self, dir: &Path) -> ContextResult<Vec<RuleFile>> {
        let mut rules = Vec::new();

        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| ContextError::Source {
                message: format!("Failed to read rules directory: {}", e),
            })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| ContextError::Source {
            message: format!("Failed to read directory entry: {}", e),
        })? {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let content = self.load_file_with_imports(&path).await?;

                rules.push(RuleFile { name, content, path });
            }
        }

        // Sort by name for consistent ordering
        rules.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(rules)
    }
}

/// Loaded memory content.
#[derive(Debug, Default)]
pub struct MemoryContent {
    /// Content from CLAUDE.md files (in order from root to current)
    pub claude_md: Vec<String>,
    /// Content from CLAUDE.local.md files
    pub local_md: Vec<String>,
    /// Rule files from .claude/rules/
    pub rules: Vec<RuleFile>,
}

impl MemoryContent {
    /// Combine all content into a single string.
    pub fn combined(&self) -> String {
        let mut parts = Vec::new();

        // CLAUDE.md content
        for content in &self.claude_md {
            if !content.trim().is_empty() {
                parts.push(content.clone());
            }
        }

        // Local content
        for content in &self.local_md {
            if !content.trim().is_empty() {
                parts.push(content.clone());
            }
        }

        // Rules content
        for rule in &self.rules {
            if !rule.content.trim().is_empty() {
                parts.push(format!("# Rule: {}\n\n{}", rule.name, rule.content));
            }
        }

        parts.join("\n\n")
    }

    /// Check if any content was loaded.
    pub fn is_empty(&self) -> bool {
        self.claude_md.is_empty() && self.local_md.is_empty() && self.rules.is_empty()
    }
}

/// A loaded rule file.
#[derive(Debug, Clone)]
pub struct RuleFile {
    /// Rule name (filename without extension)
    pub name: String,
    /// Rule content
    pub content: String,
    /// Original file path
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_load_claude_md() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Project\nTest content")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.claude_md.len(), 1);
        assert!(content.claude_md[0].contains("Test content"));
    }

    #[tokio::test]
    async fn test_load_local_md() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.local.md"), "Local settings")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.local_md.len(), 1);
        assert!(content.local_md[0].contains("Local settings"));
    }

    #[tokio::test]
    async fn test_load_rules_directory() {
        let dir = tempdir().unwrap();
        let rules_dir = dir.path().join(".claude").join("rules");
        fs::create_dir_all(&rules_dir).await.unwrap();
        fs::write(rules_dir.join("rust.md"), "# Rust Rules\nUse snake_case")
            .await
            .unwrap();
        fs::write(rules_dir.join("security.md"), "# Security\nNo secrets")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.rules.len(), 2);
    }

    #[tokio::test]
    async fn test_import_syntax() {
        let dir = tempdir().unwrap();

        // Create main file with import
        fs::write(
            dir.path().join("CLAUDE.md"),
            "# Main\n@docs/guidelines.md\nEnd",
        )
        .await
        .unwrap();

        // Create imported file
        let docs_dir = dir.path().join("docs");
        fs::create_dir_all(&docs_dir).await.unwrap();
        fs::write(docs_dir.join("guidelines.md"), "Imported content")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert!(content.combined().contains("Imported content"));
    }

    #[tokio::test]
    async fn test_combined_content() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "Main content")
            .await
            .unwrap();
        fs::write(dir.path().join("CLAUDE.local.md"), "Local content")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        let combined = content.combined();
        assert!(combined.contains("Main content"));
        assert!(combined.contains("Local content"));
    }
}
