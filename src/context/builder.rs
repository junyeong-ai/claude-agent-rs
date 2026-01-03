//! Context Builder for Progressive Disclosure

use std::path::{Path, PathBuf};

use crate::client::DEFAULT_MODEL;

use super::ContextResult;
use super::memory_loader::MemoryLoader;
use super::orchestrator::PromptOrchestrator;
use super::rule_index::{RuleIndex, RulesEngine};
use super::skill_index::SkillIndex;
use super::static_context::StaticContext;

pub struct ContextBuilder {
    system_prompt: Option<String>,
    claude_md: Option<String>,
    skill_indices: Vec<SkillIndex>,
    rule_indices: Vec<RuleIndex>,
    working_dir: Option<PathBuf>,
    model: String,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            claude_md: None,
            skill_indices: Vec::new(),
            rule_indices: Vec::new(),
            working_dir: None,
            model: DEFAULT_MODEL.to_string(),
        }
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn claude_md(mut self, content: impl Into<String>) -> Self {
        self.claude_md = Some(content.into());
        self
    }

    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(path.into());
        self
    }

    pub fn with_skill(mut self, skill: SkillIndex) -> Self {
        self.skill_indices.push(skill);
        self
    }

    pub fn with_skills(mut self, skills: impl IntoIterator<Item = SkillIndex>) -> Self {
        self.skill_indices.extend(skills);
        self
    }

    pub fn with_rule(mut self, rule: RuleIndex) -> Self {
        self.rule_indices.push(rule);
        self
    }

    pub fn with_rules(mut self, rules: impl IntoIterator<Item = RuleIndex>) -> Self {
        self.rule_indices.extend(rules);
        self
    }

    pub async fn load_from_directory(mut self, dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let mut loader = MemoryLoader::new();

        if let Ok(content) = loader.load_all(dir).await {
            let combined = content.combined_claude_md();
            if !combined.is_empty() {
                self.claude_md = Some(match self.claude_md {
                    Some(existing) => format!("{}\n\n{}", existing, combined),
                    None => combined,
                });
            }

            self.rule_indices.extend(content.rule_indices);
        }

        self
    }

    pub fn build(self) -> ContextResult<PromptOrchestrator> {
        let mut static_context = StaticContext::new();

        if let Some(ref prompt) = self.system_prompt {
            static_context = static_context.with_system_prompt(prompt.clone());
        }

        if let Some(ref md) = self.claude_md {
            static_context = static_context.with_claude_md(md.clone());
        }

        let skill_summary = self.build_skill_summary();
        if !skill_summary.is_empty() {
            static_context = static_context.with_skill_summary(skill_summary);
        }

        let mut rules_engine = RulesEngine::new();
        rules_engine.add_indices(self.rule_indices);

        let orchestrator = PromptOrchestrator::new(static_context, &self.model)
            .with_rules_engine(rules_engine)
            .with_skill_indices(self.skill_indices);

        Ok(orchestrator)
    }

    fn build_skill_summary(&self) -> String {
        if self.skill_indices.is_empty() {
            return String::new();
        }

        let mut lines = vec!["# Available Skills".to_string()];
        for skill in &self.skill_indices {
            lines.push(skill.to_summary_line());
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_builder_basic() {
        let orchestrator = ContextBuilder::new()
            .system_prompt("You are helpful")
            .claude_md("# Project\nA test project")
            .model("claude-sonnet-4-5")
            .build()
            .unwrap();

        let static_context = orchestrator.static_context();
        assert!(static_context.system_prompt.contains("helpful"));
        assert!(static_context.claude_md.contains("test project"));
    }

    #[test]
    fn test_context_builder_with_skills() {
        let skill = SkillIndex::new("test", "A test skill");

        let orchestrator = ContextBuilder::new().with_skill(skill).build().unwrap();

        assert!(!orchestrator.static_context().skill_index_summary.is_empty());
    }

    #[tokio::test]
    async fn test_load_from_directory() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Test Project")
            .await
            .unwrap();

        let rules_dir = dir.path().join(".claude").join("rules");
        fs::create_dir_all(&rules_dir).await.unwrap();
        fs::write(
            rules_dir.join("test.md"),
            r#"---
paths: **/*.rs
---

# Test Rule"#,
        )
        .await
        .unwrap();

        let orchestrator = ContextBuilder::new()
            .load_from_directory(dir.path())
            .await
            .build()
            .unwrap();

        assert!(
            orchestrator
                .static_context()
                .claude_md
                .contains("Test Project")
        );
        assert!(!orchestrator.rules_engine().indices().is_empty());
    }
}
