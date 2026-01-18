use std::path::Path;

use serde::{Deserialize, Serialize};

use super::SubagentDefinition;
use crate::common::{DocumentLoader, SourceType, is_markdown, parse_frontmatter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tools: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub skills: Option<String>,
    #[serde(default, rename = "source-type")]
    pub source_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SubagentLoader;

impl SubagentLoader {
    pub fn new() -> Self {
        Self
    }

    fn build_subagent(
        &self,
        fm: SubagentFrontmatter,
        body: String,
        _path: Option<&Path>,
    ) -> SubagentDefinition {
        let source = SourceType::from_str_opt(fm.source_type.as_deref());

        let tools: Vec<String> = fm
            .tools
            .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let skills: Vec<String> = fm
            .skills
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let mut subagent = SubagentDefinition::new(fm.name, fm.description, body)
            .with_source_type(source)
            .with_tools(tools)
            .with_skills(skills);

        if let Some(model) = fm.model {
            subagent = subagent.with_model(model);
        }

        subagent
    }
}

impl DocumentLoader<SubagentDefinition> for SubagentLoader {
    fn parse_content(
        &self,
        content: &str,
        path: Option<&Path>,
    ) -> crate::Result<SubagentDefinition> {
        let doc = parse_frontmatter::<SubagentFrontmatter>(content)?;
        Ok(self.build_subagent(doc.frontmatter, doc.body, path))
    }

    fn doc_type_name(&self) -> &'static str {
        "subagent"
    }

    fn file_filter(&self) -> fn(&Path) -> bool {
        is_markdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_subagent() {
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

        let loader = SubagentLoader::new();
        let subagent = loader.parse_content(content, None).unwrap();

        assert_eq!(subagent.name, "code-reviewer");
        assert_eq!(subagent.tools, vec!["Read", "Grep", "Glob"]);
        assert_eq!(subagent.model, Some("haiku".to_string()));
        assert!(subagent.prompt.contains("senior code reviewer"));
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

        let loader = SubagentLoader::new();
        let subagent = loader.parse_content(content, None).unwrap();

        assert_eq!(subagent.skills, vec!["security-check", "linting"]);
        assert_eq!(subagent.model, Some("sonnet".to_string()));
    }

    #[test]
    fn test_parse_without_frontmatter() {
        let content = "Just content without frontmatter";
        let loader = SubagentLoader::new();
        assert!(loader.parse_content(content, None).is_err());
    }
}
