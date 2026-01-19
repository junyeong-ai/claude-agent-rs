//! Skill index loader for progressive disclosure.
//!
//! Parses skill files to extract metadata (frontmatter) only,
//! without loading the full content into memory.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::SkillIndex;
use crate::common::{ContentSource, SourceType, is_skill_file, parse_frontmatter};

/// Frontmatter schema for skill files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default, alias = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, alias = "argument-hint")]
    pub argument_hint: Option<String>,
}

/// Loader for creating SkillIndex entries from files.
///
/// This loader extracts only the frontmatter metadata, creating a lightweight
/// index entry. The full content is loaded on-demand via ContentSource.
#[derive(Debug, Clone, Copy, Default)]
pub struct SkillIndexLoader;

impl SkillIndexLoader {
    /// Create a new skill index loader.
    pub fn new() -> Self {
        Self
    }

    /// Parse a skill file to create a SkillIndex.
    ///
    /// Only the frontmatter is parsed; the body is NOT loaded into memory.
    /// Instead, a ContentSource::File is created for lazy loading.
    pub fn parse_index(&self, content: &str, path: &Path) -> crate::Result<SkillIndex> {
        let doc = parse_frontmatter::<SkillFrontmatter>(content)?;
        Ok(self.build_index(doc.frontmatter, path))
    }

    /// Parse frontmatter only from content, returning the index.
    pub fn parse_frontmatter_only(&self, content: &str, path: &Path) -> crate::Result<SkillIndex> {
        self.parse_index(content, path)
    }

    fn build_index(&self, fm: SkillFrontmatter, path: &Path) -> SkillIndex {
        let source_type = SourceType::from_str_opt(fm.source_type.as_deref());

        let mut index = SkillIndex::new(fm.name, fm.description)
            .with_source(ContentSource::file(path))
            .with_source_type(source_type);

        if !fm.triggers.is_empty() {
            index = index.with_triggers(fm.triggers);
        }

        if !fm.allowed_tools.is_empty() {
            index = index.with_allowed_tools(fm.allowed_tools);
        }

        if let Some(model) = fm.model {
            index = index.with_model(model);
        }

        if let Some(hint) = fm.argument_hint {
            index = index.with_argument_hint(hint);
        }

        index
    }

    /// Load a skill index from a file.
    pub async fn load_file(&self, path: &Path) -> crate::Result<SkillIndex> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            crate::Error::Config(format!("Failed to read skill file {:?}: {}", path, e))
        })?;

        self.parse_index(&content, path)
    }

    /// Scan a directory for skill files and create indices.
    ///
    /// This recursively scans the directory for skill files (.skill.md or SKILL.md)
    /// and creates SkillIndex entries for each.
    pub async fn scan_directory(&self, dir: &Path) -> crate::Result<Vec<SkillIndex>> {
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
        indices: &'a mut Vec<SkillIndex>,
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
                    // Check for SKILL.md in subdirectory (skill folder pattern)
                    let skill_file = path.join("SKILL.md");
                    if skill_file.exists() {
                        if let Ok(index) = self.load_file(&skill_file).await {
                            indices.push(index);
                        }
                    } else {
                        // Recurse into subdirectory
                        self.scan_directory_recursive(&path, indices).await?;
                    }
                } else if is_skill_file(&path)
                    && let Ok(index) = self.load_file(&path).await
                {
                    indices.push(index);
                }
            }

            Ok(())
        })
    }

    /// Create an inline skill index with content already available.
    pub fn create_inline(
        &self,
        name: impl Into<String>,
        description: impl Into<String>,
        content: impl Into<String>,
    ) -> SkillIndex {
        SkillIndex::new(name, description).with_source(ContentSource::in_memory(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
triggers:
  - /test
  - test please
allowed-tools:
  - Read
  - Grep
model: claude-haiku-4-5-20251001
---

This is the skill content that should NOT be loaded into memory during indexing.
"#;

        let loader = SkillIndexLoader::new();
        let index = loader
            .parse_index(content, Path::new("/skills/test.skill.md"))
            .unwrap();

        assert_eq!(index.name, "test-skill");
        assert_eq!(index.description, "A test skill");
        assert_eq!(index.triggers, vec!["/test", "test please"]);
        assert_eq!(index.allowed_tools, vec!["Read", "Grep"]);
        assert_eq!(index.model, Some("claude-haiku-4-5-20251001".to_string()));

        // Verify source is file-based (for lazy loading)
        assert!(index.source.is_file());
    }

    #[test]
    fn test_parse_minimal_frontmatter() {
        let content = r#"---
name: minimal
description: Minimal skill
---

Content here.
"#;

        let loader = SkillIndexLoader::new();
        let index = loader
            .parse_index(content, Path::new("/skills/minimal.skill.md"))
            .unwrap();

        assert_eq!(index.name, "minimal");
        assert!(index.triggers.is_empty());
        assert!(index.allowed_tools.is_empty());
        assert!(index.model.is_none());
    }

    #[test]
    fn test_create_inline() {
        let loader = SkillIndexLoader::new();
        let index = loader.create_inline("inline", "Inline skill", "Full content");

        assert_eq!(index.name, "inline");
        assert!(index.source.is_in_memory());
    }

    #[tokio::test]
    async fn test_load_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::with_suffix(".skill.md").unwrap();
        writeln!(
            file,
            r#"---
name: file-skill
description: From file
---

Content."#
        )
        .unwrap();

        let loader = SkillIndexLoader::new();
        let index = loader.load_file(file.path()).await.unwrap();

        assert_eq!(index.name, "file-skill");
        assert!(index.source.is_file());
    }

    #[tokio::test]
    async fn test_scan_directory() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();

        // Create test skill files
        fs::write(
            dir.path().join("skill1.skill.md"),
            r#"---
name: skill1
description: First skill
---
Content 1"#,
        )
        .await
        .unwrap();

        fs::write(
            dir.path().join("skill2.skill.md"),
            r#"---
name: skill2
description: Second skill
---
Content 2"#,
        )
        .await
        .unwrap();

        let loader = SkillIndexLoader::new();
        let indices = loader.scan_directory(dir.path()).await.unwrap();

        assert_eq!(indices.len(), 2);
        let names: Vec<&str> = indices.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"skill1"));
        assert!(names.contains(&"skill2"));
    }

    #[tokio::test]
    async fn test_scan_directory_with_skill_folder() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();

        // Create skill folder pattern
        let skill_dir = dir.path().join("my-skill");
        fs::create_dir(&skill_dir).await.unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: folder-skill
description: From folder
---
Content"#,
        )
        .await
        .unwrap();

        let loader = SkillIndexLoader::new();
        let indices = loader.scan_directory(dir.path()).await.unwrap();

        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0].name, "folder-skill");
    }

    #[tokio::test]
    async fn test_scan_nonexistent_directory() {
        let loader = SkillIndexLoader::new();
        let indices = loader
            .scan_directory(Path::new("/nonexistent/path"))
            .await
            .unwrap();
        assert!(indices.is_empty());
    }
}
