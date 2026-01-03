//! CLAUDE.md Memory Loader
//!
//! Implements Claude Code CLI compatible memory loading:
//! - Recursive loading from current directory to root
//! - CLAUDE.local.md support
//! - @import syntax (max 5 hops)
//! - .claude/rules/ index-only loading for progressive disclosure

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::provider::MAX_IMPORT_DEPTH;
use super::rule_index::RuleIndex;
use super::{ContextError, ContextResult};

#[derive(Debug, Default)]
pub struct MemoryLoader {
    loaded_paths: HashSet<PathBuf>,
    current_depth: usize,
}

impl MemoryLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn load_all(&mut self, start_dir: &Path) -> ContextResult<MemoryContent> {
        let mut content = MemoryContent::default();

        let claude_files = self.find_claude_files(start_dir);
        for path in claude_files {
            if let Ok(text) = self.load_file_with_imports(&path).await {
                content.claude_md.push(text);
            }
        }

        let local_files = self.find_local_files(start_dir);
        for path in local_files {
            if let Ok(text) = self.load_file_with_imports(&path).await {
                content.local_md.push(text);
            }
        }

        let rules_dir = start_dir.join(".claude").join("rules");
        if rules_dir.exists() {
            content.rule_indices = self.scan_rules_directory(&rules_dir).await?;
        }

        Ok(content)
    }

    pub async fn load_local_only(&mut self, start_dir: &Path) -> ContextResult<MemoryContent> {
        let mut content = MemoryContent::default();

        let local_files = self.find_local_files(start_dir);
        for path in local_files {
            if let Ok(text) = self.load_file_with_imports(&path).await {
                content.local_md.push(text);
            }
        }

        Ok(content)
    }

    fn find_claude_files(&self, start_dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut current = start_dir.to_path_buf();

        loop {
            let claude_md = current.join("CLAUDE.md");
            if claude_md.exists() {
                files.push(claude_md);
            }

            let claude_dir_md = current.join(".claude").join("CLAUDE.md");
            if claude_dir_md.exists() {
                files.push(claude_dir_md);
            }

            match current.parent() {
                Some(parent) if parent != current && !parent.as_os_str().is_empty() => {
                    current = parent.to_path_buf();
                }
                _ => break,
            }
        }

        files.reverse();
        files
    }

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

            match current.parent() {
                Some(parent) if parent != current && !parent.as_os_str().is_empty() => {
                    current = parent.to_path_buf();
                }
                _ => break,
            }
        }

        files.reverse();
        files
    }

    async fn scan_rules_directory(&self, dir: &Path) -> ContextResult<Vec<RuleIndex>> {
        let mut indices = Vec::new();

        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| ContextError::Source {
                message: format!("Failed to read rules directory: {}", e),
            })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ContextError::Source {
                message: format!("Failed to read directory entry: {}", e),
            })?
        {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md")
                && let Some(index) = RuleIndex::from_file(&path)
            {
                indices.push(index);
            }
        }

        indices.sort_by(|a, b| b.priority.cmp(&a.priority));
        Ok(indices)
    }

    fn load_file_with_imports<'a>(
        &'a mut self,
        path: &'a Path,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ContextResult<String>> + Send + 'a>>
    {
        Box::pin(async move {
            if self.current_depth >= MAX_IMPORT_DEPTH {
                tracing::warn!(
                    "Import depth limit ({}) reached, skipping: {}",
                    MAX_IMPORT_DEPTH,
                    path.display()
                );
                return Ok(String::new());
            }

            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            if self.loaded_paths.contains(&canonical) {
                return Ok(String::new());
            }
            self.loaded_paths.insert(canonical.clone());

            let content =
                tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| ContextError::Source {
                        message: format!("Failed to read {}: {}", path.display(), e),
                    })?;

            self.current_depth += 1;
            let result = self
                .process_imports(&content, path.parent().unwrap_or(Path::new(".")))
                .await;
            self.current_depth -= 1;

            result
        })
    }

    fn expand_home(path: &str) -> PathBuf {
        if let Some(rest) = path.strip_prefix("~/")
            && let Some(home) = crate::common::home_dir()
        {
            return home.join(rest);
        }
        PathBuf::from(path)
    }

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
}

#[derive(Debug, Default)]
pub struct MemoryContent {
    pub claude_md: Vec<String>,
    pub local_md: Vec<String>,
    pub rule_indices: Vec<RuleIndex>,
}

impl MemoryContent {
    pub fn combined_claude_md(&self) -> String {
        let mut parts = Vec::new();

        for content in &self.claude_md {
            if !content.trim().is_empty() {
                parts.push(content.clone());
            }
        }

        for content in &self.local_md {
            if !content.trim().is_empty() {
                parts.push(content.clone());
            }
        }

        parts.join("\n\n")
    }

    pub fn is_empty(&self) -> bool {
        self.claude_md.is_empty() && self.local_md.is_empty() && self.rule_indices.is_empty()
    }
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
    async fn test_scan_rules_indices_only() {
        let dir = tempdir().unwrap();
        let rules_dir = dir.path().join(".claude").join("rules");
        fs::create_dir_all(&rules_dir).await.unwrap();

        fs::write(
            rules_dir.join("rust.md"),
            r#"---
paths: **/*.rs
priority: 10
---

# Rust Rules
Use snake_case"#,
        )
        .await
        .unwrap();

        fs::write(rules_dir.join("security.md"), "# Security\nNo secrets")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.rule_indices.len(), 2);

        let rust_rule = content.rule_indices.iter().find(|r| r.name == "rust");
        assert!(rust_rule.is_some());
        assert_eq!(rust_rule.unwrap().priority, 10);
        assert!(rust_rule.unwrap().paths.is_some());
    }

    #[tokio::test]
    async fn test_import_syntax() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("CLAUDE.md"),
            "# Main\n@docs/guidelines.md\nEnd",
        )
        .await
        .unwrap();

        let docs_dir = dir.path().join("docs");
        fs::create_dir_all(&docs_dir).await.unwrap();
        fs::write(docs_dir.join("guidelines.md"), "Imported content")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert!(content.combined_claude_md().contains("Imported content"));
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

        let combined = content.combined_claude_md();
        assert!(combined.contains("Main content"));
        assert!(combined.contains("Local content"));
    }
}
