//! Subagent index loader.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::SubagentIndex;
use crate::client::ModelType;
use crate::common::{ContentSource, SourceType, is_markdown, parse_frontmatter};
use crate::hooks::HookRule;

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
    #[serde(default, alias = "disallowedTools")]
    pub disallowed_tools: Option<String>,
    #[serde(default, alias = "permissionMode")]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub hooks: Option<HashMap<String, Vec<HookRule>>>,
}

fn split_csv(s: Option<String>) -> Vec<String> {
    s.map(|v| {
        let mut items = Vec::new();
        let mut current = String::new();
        let mut depth = 0u32;
        for ch in v.chars() {
            match ch {
                '(' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' => {
                    depth = depth.saturating_sub(1);
                    current.push(ch);
                }
                ',' if depth == 0 => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        items.push(trimmed);
                    }
                    current.clear();
                }
                _ => current.push(ch),
            }
        }
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            items.push(trimmed);
        }
        items
    })
    .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SubagentIndexLoader;

impl SubagentIndexLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_index(&self, content: &str, path: &Path) -> crate::Result<SubagentIndex> {
        let doc = parse_frontmatter::<SubagentFrontmatter>(content)?;
        Ok(self.build_index(doc.frontmatter, path))
    }

    fn build_index(&self, fm: SubagentFrontmatter, path: &Path) -> SubagentIndex {
        let source_type = SourceType::from_str_opt(fm.source_type.as_deref());

        let tools = split_csv(fm.tools);
        let skills = split_csv(fm.skills);
        let disallowed_tools = split_csv(fm.disallowed_tools);

        let mut index = SubagentIndex::new(fm.name, fm.description)
            .source(ContentSource::file(path))
            .source_type(source_type)
            .tools(tools)
            .skills(skills);

        index.disallowed_tools = disallowed_tools;
        index.permission_mode = fm.permission_mode;
        index.hooks = fm.hooks;

        if let Some(m) = fm.model {
            index = index.model(m);
        }

        if let Some(mt) = fm.model_type {
            match mt.to_lowercase().as_str() {
                "small" | "haiku" => index = index.model_type(ModelType::Small),
                "primary" | "sonnet" => index = index.model_type(ModelType::Primary),
                "reasoning" | "opus" => index = index.model_type(ModelType::Reasoning),
                _ => {}
            }
        }

        index
    }

    /// Load a subagent index from a file.
    pub async fn load_file(&self, path: &Path) -> crate::Result<SubagentIndex> {
        crate::common::index_loader::load_file(path, |c, p| self.parse_index(c, p), "subagent")
            .await
    }

    /// Scan a directory for subagent files and create indices.
    pub async fn scan_directory(&self, dir: &Path) -> crate::Result<Vec<SubagentIndex>> {
        use crate::common::index_loader::{self, DirAction};

        let loader = Self::new();
        index_loader::scan_directory(
            dir,
            |p| Box::pin(async move { loader.load_file(p).await }),
            is_markdown,
            |_| DirAction::Recurse,
        )
        .await
    }

    /// Create an inline subagent index with in-memory content.
    pub fn create_inline(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
    ) -> SubagentIndex {
        SubagentIndex::new(name, description).source(ContentSource::in_memory(prompt))
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

    #[test]
    fn test_parse_disallowed_tools() {
        let content = r#"---
name: restricted-agent
description: Agent with disallowed tools
disallowedTools: Write, Edit
---
Restricted prompt"#;

        let loader = SubagentIndexLoader::new();
        let index = loader
            .parse_index(content, Path::new("/test/restricted.md"))
            .unwrap();

        assert_eq!(index.disallowed_tools, vec!["Write", "Edit"]);
    }

    #[test]
    fn test_parse_permission_mode() {
        let content = r#"---
name: auto-agent
description: Agent with permission mode
permissionMode: dontAsk
---
Auto prompt"#;

        let loader = SubagentIndexLoader::new();
        let index = loader
            .parse_index(content, Path::new("/test/auto.md"))
            .unwrap();

        assert_eq!(index.permission_mode, Some("dontAsk".to_string()));
    }

    #[test]
    fn test_split_csv_with_parens() {
        let result = split_csv(Some("Read, Bash(git:*,docker:*), Write".to_string()));
        assert_eq!(result, vec!["Read", "Bash(git:*,docker:*)", "Write"]);
    }

    #[test]
    fn test_split_csv_simple() {
        let result = split_csv(Some("Read, Grep, Glob".to_string()));
        assert_eq!(result, vec!["Read", "Grep", "Glob"]);
    }

    #[test]
    fn test_defaults_for_new_subagent_fields() {
        let content = r#"---
name: basic-agent
description: Basic agent
---
Prompt"#;

        let loader = SubagentIndexLoader::new();
        let index = loader
            .parse_index(content, Path::new("/test/basic.md"))
            .unwrap();

        assert!(index.disallowed_tools.is_empty());
        assert!(index.permission_mode.is_none());
    }
}
