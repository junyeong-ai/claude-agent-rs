use std::path::Path;

use serde::{Deserialize, Serialize};

use super::SkillDefinition;
use crate::common::{DocumentLoader, SourceType, is_skill_file, parse_frontmatter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default, alias = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SkillLoader;

impl SkillLoader {
    pub fn new() -> Self {
        Self
    }

    fn build_skill(
        &self,
        fm: SkillFrontmatter,
        body: String,
        path: Option<&Path>,
    ) -> SkillDefinition {
        let source_type = SourceType::from_str_opt(fm.source_type.as_deref());

        let mut skill =
            SkillDefinition::new(fm.name, fm.description, body).with_source_type(source_type);

        if let Some(p) = path
            && let Some(parent) = p.parent()
        {
            skill = skill.with_base_dir(parent);
        }

        for trigger in fm.triggers {
            skill = skill.with_trigger(trigger);
        }

        if let Some(args) = fm.arguments {
            skill = skill.with_arguments(args);
        }

        if !fm.allowed_tools.is_empty() {
            skill = skill.with_allowed_tools(fm.allowed_tools);
        }

        if let Some(model) = fm.model {
            skill = skill.with_model(model);
        }

        skill
    }
}

impl DocumentLoader<SkillDefinition> for SkillLoader {
    fn parse_content(&self, content: &str, path: Option<&Path>) -> crate::Result<SkillDefinition> {
        let doc = parse_frontmatter::<SkillFrontmatter>(content)?;
        Ok(self.build_skill(doc.frontmatter, doc.body, path))
    }

    fn doc_type_name(&self) -> &'static str {
        "skill"
    }

    fn file_filter(&self) -> fn(&Path) -> bool {
        is_skill_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ToolRestricted;
    use crate::skills::SkillSourceType;

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
        let skill = loader.parse_content(content, None).unwrap();

        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "A test skill");
        assert!(skill.content.contains("This is the skill content"));
        assert_eq!(skill.triggers.len(), 2);
    }

    #[test]
    fn test_parse_skill_without_frontmatter() {
        let content = "Just some content without frontmatter";
        let loader = SkillLoader::new();
        let result = loader.parse_content(content, None);

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
        let skill = loader.parse_content(content, None).unwrap();

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
        let skill = loader.parse_content(content, None).unwrap();

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
        let skill = loader.parse_content(content, None).unwrap();

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
        let skill = loader.parse_content(content, None).unwrap();

        assert_eq!(skill.name, "full-skill");
        assert_eq!(skill.source_type, SkillSourceType::Project);
        assert_eq!(skill.allowed_tools, vec!["Read", "Bash(git:*)"]);
        assert_eq!(skill.model, Some("claude-opus-4-5-20251101".to_string()));
        assert!(skill.matches_trigger("/full please"));
    }

    #[test]
    fn test_load_inline() {
        let content = r#"---
name: inline-skill
description: An inline skill
---

Inline content.
"#;

        let loader = SkillLoader::new();
        let skill = loader.load_inline(content).unwrap();

        assert_eq!(skill.name, "inline-skill");
    }
}
