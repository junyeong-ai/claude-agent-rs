//! Skill loader - parses SKILL.md files with YAML frontmatter.
//!
//! Skills can be defined in markdown files with YAML frontmatter containing
//! metadata like name, description, and triggers.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{SkillDefinition, SkillSourceType};

/// Source location for loading skills
#[derive(Debug, Clone)]
pub enum SkillSource {
    /// Load from a file path
    File(std::path::PathBuf),
    /// Load from a string (inline definition)
    Inline(String),
    /// Load from a directory (scan for SKILL.md files)
    Directory(std::path::PathBuf),
}

/// YAML frontmatter structure for SKILL.md files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    /// Skill name (required)
    pub name: String,
    /// Short description (required)
    pub description: String,
    /// Optional trigger patterns
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Optional arguments schema
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
    /// Optional source type override
    #[serde(default)]
    pub source_type: Option<String>,
    /// Allowed tools for this skill (security boundary)
    #[serde(default, alias = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    /// Model override for cost optimization
    #[serde(default)]
    pub model: Option<String>,
}

/// Loader for skill definitions from various sources
#[derive(Debug)]
pub struct SkillLoader;

impl SkillLoader {
    /// Create a new skill loader
    pub fn new() -> Self {
        Self
    }

    /// Load a skill from a file path
    pub async fn load_file(&self, path: &Path) -> crate::Result<SkillDefinition> {
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            crate::Error::Config(format!(
                "Failed to read skill file {}: {}",
                path.display(),
                e
            ))
        })?;

        self.parse_skill_content(&content, Some(path))
    }

    /// Load skills from a directory
    pub async fn load_directory(&self, dir: &Path) -> crate::Result<Vec<SkillDefinition>> {
        let mut skills = Vec::new();

        let mut entries = tokio::fs::read_dir(dir).await.map_err(|e| {
            crate::Error::Config(format!("Failed to read directory {}: {}", dir.display(), e))
        })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| crate::Error::Config(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();

            // Check for SKILL.md files or .skill.md suffix
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && (name.eq_ignore_ascii_case("SKILL.md") || name.ends_with(".skill.md"))
            {
                match self.load_file(&path).await {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        tracing::warn!("Failed to load skill from {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(skills)
    }

    /// Parse skill content from a string
    pub fn parse_skill_content(
        &self,
        content: &str,
        path: Option<&Path>,
    ) -> crate::Result<SkillDefinition> {
        // Check for YAML frontmatter (--- delimited)
        if content.starts_with("---") {
            self.parse_with_frontmatter(content, path)
        } else {
            Err(crate::Error::Config(
                "Skill file must have YAML frontmatter (starting with ---)".to_string(),
            ))
        }
    }

    /// Parse a skill with YAML frontmatter
    fn parse_with_frontmatter(
        &self,
        content: &str,
        path: Option<&Path>,
    ) -> crate::Result<SkillDefinition> {
        // Find the end of frontmatter
        let after_first_delimiter = &content[3..];
        let end_pos = after_first_delimiter.find("---").ok_or_else(|| {
            crate::Error::Config(
                "Skill file frontmatter not properly terminated with ---".to_string(),
            )
        })?;

        let frontmatter_str = &after_first_delimiter[..end_pos].trim();
        let body = &after_first_delimiter[end_pos + 3..].trim();

        // Parse YAML frontmatter
        let frontmatter: SkillFrontmatter = serde_yaml::from_str(frontmatter_str).map_err(|e| {
            crate::Error::Config(format!("Failed to parse skill frontmatter: {}", e))
        })?;

        // Determine source type
        let source_type = match frontmatter.source_type.as_deref() {
            Some("builtin") => SkillSourceType::Builtin,
            Some("project") => SkillSourceType::Project,
            Some("managed") => SkillSourceType::Managed,
            _ => SkillSourceType::User,
        };

        let mut skill =
            SkillDefinition::new(frontmatter.name, frontmatter.description, body.to_string())
                .with_source_type(source_type);

        // Add location if available
        if let Some(p) = path {
            skill = skill.with_location(p.display().to_string());
        }

        // Add triggers
        for trigger in frontmatter.triggers {
            skill = skill.with_trigger(trigger);
        }

        // Add arguments schema
        if let Some(args) = frontmatter.arguments {
            skill = skill.with_arguments(args);
        }

        // Add allowed tools (security boundary)
        if !frontmatter.allowed_tools.is_empty() {
            skill = skill.with_allowed_tools(frontmatter.allowed_tools);
        }

        // Add model override (cost optimization)
        if let Some(model) = frontmatter.model {
            skill = skill.with_model(model);
        }

        Ok(skill)
    }

    /// Load a skill from inline content
    pub fn load_inline(&self, content: &str) -> crate::Result<SkillDefinition> {
        self.parse_skill_content(content, None)
    }
}

impl Default for SkillLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_with_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
triggers:
  - /test
  - test please
---

This is the skill content.

It can have multiple paragraphs.
"#;

        let loader = SkillLoader::new();
        let skill = loader.parse_skill_content(content, None).unwrap();

        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "A test skill");
        assert!(skill.content.contains("This is the skill content"));
        assert_eq!(skill.triggers.len(), 2);
    }

    #[test]
    fn test_parse_skill_without_frontmatter() {
        let content = "Just some content without frontmatter";
        let loader = SkillLoader::new();
        let result = loader.parse_skill_content(content, None);

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_skill_with_source_type() {
        let content = r#"---
name: builtin-skill
description: A builtin skill
source_type: builtin
---

Content here.
"#;

        let loader = SkillLoader::new();
        let skill = loader.parse_skill_content(content, None).unwrap();

        assert_eq!(skill.source_type, SkillSourceType::Builtin);
    }

    #[test]
    fn test_parse_skill_with_allowed_tools() {
        let content = r#"---
name: reader-skill
description: Read-only skill
allowed-tools:
  - Read
  - Grep
  - Glob
---

Read files only.
"#;

        let loader = SkillLoader::new();
        let skill = loader.parse_skill_content(content, None).unwrap();

        assert_eq!(skill.allowed_tools, vec!["Read", "Grep", "Glob"]);
        assert!(skill.has_tool_restrictions());
        assert!(skill.is_tool_allowed("Read"));
        assert!(!skill.is_tool_allowed("Bash"));
    }

    #[test]
    fn test_parse_skill_with_model() {
        let content = r#"---
name: fast-skill
description: Fast processing
model: claude-haiku-4-5-20251001
---

Quick task.
"#;

        let loader = SkillLoader::new();
        let skill = loader.parse_skill_content(content, None).unwrap();

        assert_eq!(skill.model, Some("claude-haiku-4-5-20251001".to_string()));
    }

    #[test]
    fn test_parse_skill_with_all_options() {
        let content = r#"---
name: full-skill
description: Full featured skill
triggers:
  - /full
allowed-tools:
  - Read
  - Bash(git:*)
model: claude-opus-4-5-20251101
source_type: project
---

Full skill content.
"#;

        let loader = SkillLoader::new();
        let skill = loader.parse_skill_content(content, None).unwrap();

        assert_eq!(skill.name, "full-skill");
        assert_eq!(skill.source_type, SkillSourceType::Project);
        assert_eq!(skill.allowed_tools, vec!["Read", "Bash(git:*)"]);
        assert_eq!(skill.model, Some("claude-opus-4-5-20251101".to_string()));
        assert!(skill.matches_trigger("/full please"));
    }
}
