//! Subagent Index Loader - loads only metadata, defers prompt loading.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::SubagentIndex;
use crate::client::ModelType;
use crate::common::{ContentSource, SourceType, is_markdown, parse_frontmatter};

/// Frontmatter for subagent files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tools: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub model_type: Option<String>,
    #[serde(default)]
    pub skills: Option<String>,
    #[serde(default, rename = "source-type")]
    pub source_type: Option<String>,
}

/// Loader for SubagentIndex - only loads metadata, not full prompt.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubagentIndexLoader;

impl SubagentIndexLoader {
    pub fn new() -> Self {
        Self
    }

    /// Parse a subagent file and create an index (metadata only).
    /// The prompt content is NOT loaded - it will be loaded lazily via ContentSource.
    pub fn parse_index(&self, content: &str, path: &Path) -> crate::Result<SubagentIndex> {
        let doc = parse_frontmatter::<SubagentFrontmatter>(content)?;
        Ok(self.build_index(doc.frontmatter, path))
    }

    /// Parse frontmatter only from content, returning the index.
    pub fn parse_frontmatter_only(
        &self,
        content: &str,
        path: &Path,
    ) -> crate::Result<SubagentIndex> {
        self.parse_index(content, path)
    }

    fn build_index(&self, fm: SubagentFrontmatter, path: &Path) -> SubagentIndex {
        let source_type = SourceType::from_str_opt(fm.source_type.as_deref());

        let tools: Vec<String> = fm
            .tools
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let skills: Vec<String> = fm
            .skills
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let mut index = SubagentIndex::new(fm.name, fm.description)
            .with_source(ContentSource::file(path))
            .with_source_type(source_type)
            .with_tools(tools)
            .with_skills(skills);

        if let Some(model) = fm.model {
            index = index.with_model(model);
        }

        if let Some(model_type) = fm.model_type {
            match model_type.to_lowercase().as_str() {
                "small" | "haiku" => index = index.with_model_type(ModelType::Small),
                "primary" | "sonnet" => index = index.with_model_type(ModelType::Primary),
                "reasoning" | "opus" => index = index.with_model_type(ModelType::Reasoning),
                _ => {}
            }
        }

        index
    }

    /// Load a subagent index from a file.
    pub async fn load_file(&self, path: &Path) -> crate::Result<SubagentIndex> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            crate::Error::Config(format!("Failed to read subagent file {:?}: {}", path, e))
        })?;

        self.parse_index(&content, path)
    }

    /// Scan a directory for subagent files and create indices.
    pub async fn scan_directory(&self, dir: &Path) -> crate::Result<Vec<SubagentIndex>> {
        let mut indices = Vec::new();

        if !dir.exists() {
            return Ok(indices);
        }

        self.scan_directory_recursive(dir, &mut indices).await?;
        Ok(indices)
    }

    fn scan_directory_recursive<'a>(
        &'a self,
        dir: &'a Path,
        indices: &'a mut Vec<SubagentIndex>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(dir).await.map_err(|e| {
                crate::Error::Config(format!("Failed to read directory {:?}: {}", dir, e))
            })?;

            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                crate::Error::Config(format!("Failed to read directory entry: {}", e))
            })? {
                let path = entry.path();

                if path.is_dir() {
                    self.scan_directory_recursive(&path, indices).await?;
                } else if is_markdown(&path) {
                    match self.load_file(&path).await {
                        Ok(index) => indices.push(index),
                        Err(e) => {
                            tracing::warn!("Failed to load subagent from {:?}: {}", path, e);
                        }
                    }
                }
            }

            Ok(())
        })
    }

    /// Create an inline subagent index with in-memory content.
    pub fn create_inline(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
    ) -> SubagentIndex {
        SubagentIndex::new(name, description).with_source(ContentSource::in_memory(prompt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subagent_index() {
        let content = r#"---
name: code-reviewer
description: Expert code reviewer for quality checks
tools: Read, Grep, Glob
model: haiku
---

You are a senior code reviewer focusing on:
- Code quality and best practices
- Security vulnerabilities
"#;

        let loader = SubagentIndexLoader::new();
        let index = loader
            .parse_index(content, Path::new("/test/reviewer.md"))
            .unwrap();

        assert_eq!(index.name, "code-reviewer");
        assert_eq!(index.description, "Expert code reviewer for quality checks");
        assert_eq!(index.allowed_tools, vec!["Read", "Grep", "Glob"]);
        assert_eq!(index.model, Some("haiku".to_string()));
        // Note: prompt content is NOT loaded here - it's in ContentSource
        assert!(index.source.is_file());
    }

    #[test]
    fn test_parse_subagent_with_skills() {
        let content = r#"---
name: full-agent
description: Full featured agent
tools: Read, Write, Bash(git:*)
model: sonnet
skills: security-check, linting
---

Full agent prompt.
"#;

        let loader = SubagentIndexLoader::new();
        let index = loader
            .parse_index(content, Path::new("/test/full.md"))
            .unwrap();

        assert_eq!(index.skills, vec!["security-check", "linting"]);
        assert_eq!(index.model, Some("sonnet".to_string()));
    }

    #[test]
    fn test_create_inline() {
        let index = SubagentIndexLoader::create_inline(
            "test-agent",
            "Test description",
            "You are a test agent.",
        );

        assert_eq!(index.name, "test-agent");
        assert!(index.source.is_in_memory());
    }

    #[test]
    fn test_parse_without_frontmatter() {
        let content = "Just content without frontmatter";
        let loader = SubagentIndexLoader::new();
        assert!(loader.parse_index(content, Path::new("/test.md")).is_err());
    }
}
