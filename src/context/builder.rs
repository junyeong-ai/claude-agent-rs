//! Context Builder for Dynamic Context Injection
//!
//! Provides a fluent API for constructing context with support for
//! multiple sources and dynamic content.

use std::path::{Path, PathBuf};

use super::memory_loader::MemoryLoader;
use super::orchestrator::ContextOrchestrator;
use super::rule_index::RuleIndex;
use super::skill_index::SkillIndex;
use super::static_context::StaticContext;
use super::{ContextError, ContextResult};

/// Builder for constructing context configuration
pub struct ContextBuilder {
    /// System prompt override
    system_prompt: Option<String>,

    /// CLAUDE.md content or path
    claude_md: Option<String>,

    /// Tool definitions (names only for summary)
    tool_names: Vec<String>,

    /// Skill indices to register
    skill_indices: Vec<SkillIndex>,

    /// Rule indices to register
    rule_indices: Vec<RuleIndex>,

    /// Dynamic context parts (evaluated at build time)
    dynamic_parts: Vec<Box<dyn Fn() -> String + Send + Sync>>,

    /// Conditional context parts
    conditional_parts: Vec<(Box<dyn Fn() -> bool + Send + Sync>, String)>,

    /// File sources to load
    file_sources: Vec<PathBuf>,

    /// Model for context window sizing
    model: String,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextBuilder {
    /// Create a new context builder
    pub fn new() -> Self {
        Self {
            system_prompt: None,
            claude_md: None,
            tool_names: Vec::new(),
            skill_indices: Vec::new(),
            rule_indices: Vec::new(),
            dynamic_parts: Vec::new(),
            conditional_parts: Vec::new(),
            file_sources: Vec::new(),
            model: "claude-sonnet-4-5".to_string(),
        }
    }

    /// Set the model (affects context window size)
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the system prompt
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set CLAUDE.md content directly
    pub fn claude_md(mut self, content: impl Into<String>) -> Self {
        self.claude_md = Some(content.into());
        self
    }

    /// Load CLAUDE.md from a file path
    pub fn claude_md_from_file(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.claude_md = Some(content);
            }
        }
        self
    }

    /// Load all memory files recursively from a directory.
    ///
    /// This implements Claude Code CLI compatible loading:
    /// - CLAUDE.md files from current directory to root
    /// - CLAUDE.local.md files (private settings)
    /// - .claude/rules/*.md files
    /// - @import syntax for file inclusion
    pub async fn load_memory_recursive(mut self, start_dir: impl AsRef<Path>) -> Self {
        let mut loader = MemoryLoader::new();
        if let Ok(content) = loader.load_all(start_dir.as_ref()).await {
            let combined = content.combined();
            if !combined.is_empty() {
                self.claude_md = Some(combined);
            }
        }
        self
    }

    /// Add a file-based context source (.claude directory)
    pub fn with_file_source(mut self, path: impl AsRef<Path>) -> Self {
        self.file_sources.push(path.as_ref().to_path_buf());
        self
    }

    /// Add tool names for summary
    pub fn with_tools(mut self, tool_names: Vec<String>) -> Self {
        self.tool_names.extend(tool_names);
        self
    }

    /// Add a skill index
    pub fn with_skill(mut self, skill: SkillIndex) -> Self {
        self.skill_indices.push(skill);
        self
    }

    /// Add multiple skill indices
    pub fn with_skills(mut self, skills: Vec<SkillIndex>) -> Self {
        self.skill_indices.extend(skills);
        self
    }

    /// Add a rule index
    pub fn with_rule(mut self, rule: RuleIndex) -> Self {
        self.rule_indices.push(rule);
        self
    }

    /// Add multiple rule indices
    pub fn with_rules(mut self, rules: Vec<RuleIndex>) -> Self {
        self.rule_indices.extend(rules);
        self
    }

    /// Add dynamic context (evaluated at build time)
    pub fn with_dynamic<F>(mut self, f: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.dynamic_parts.push(Box::new(f));
        self
    }

    /// Add conditional context
    pub fn when<F>(mut self, condition: F, content: impl Into<String>) -> Self
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        self.conditional_parts
            .push((Box::new(condition), content.into()));
        self
    }

    /// Build the context orchestrator
    pub async fn build(self) -> ContextResult<ContextOrchestrator> {
        // Build static context
        let mut static_ctx = StaticContext::new();

        // Set system prompt
        if let Some(ref prompt) = self.system_prompt {
            static_ctx = static_ctx.with_system_prompt(prompt.clone());
        }

        // Set CLAUDE.md
        if let Some(ref md) = self.claude_md {
            static_ctx = static_ctx.with_claude_md(md.clone());
        }

        // Load from file sources
        for source_path in &self.file_sources {
            self.load_file_source(&mut static_ctx, source_path).await?;
        }

        // Build skill summary
        let skill_summary = self.build_skill_summary();
        if !skill_summary.is_empty() {
            static_ctx = static_ctx.with_skill_summary(skill_summary);
        }

        // Evaluate dynamic parts
        let mut dynamic_content = Vec::new();
        for part_fn in &self.dynamic_parts {
            dynamic_content.push(part_fn());
        }

        // Evaluate conditional parts
        for (condition, content) in &self.conditional_parts {
            if condition() {
                dynamic_content.push(content.clone());
            }
        }

        // Append dynamic content to CLAUDE.md
        if !dynamic_content.is_empty() {
            let existing = static_ctx.claude_md.clone();
            let combined = if existing.is_empty() {
                dynamic_content.join("\n\n")
            } else {
                format!("{}\n\n{}", existing, dynamic_content.join("\n\n"))
            };
            static_ctx = static_ctx.with_claude_md(combined);
        }

        // Create orchestrator
        let mut orchestrator = ContextOrchestrator::new(static_ctx, &self.model);

        // Add skill and rule indices
        orchestrator.add_skill_indices(self.skill_indices);
        orchestrator.add_rule_indices(self.rule_indices);

        Ok(orchestrator)
    }

    /// Build synchronously (for simpler cases)
    pub fn build_sync(self) -> ContextResult<ContextOrchestrator> {
        // Build static context
        let mut static_ctx = StaticContext::new();

        // Set system prompt
        if let Some(ref prompt) = self.system_prompt {
            static_ctx = static_ctx.with_system_prompt(prompt.clone());
        }

        // Set CLAUDE.md
        if let Some(ref md) = self.claude_md {
            static_ctx = static_ctx.with_claude_md(md.clone());
        }

        // Build skill summary
        let skill_summary = self.build_skill_summary();
        if !skill_summary.is_empty() {
            static_ctx = static_ctx.with_skill_summary(skill_summary);
        }

        // Evaluate dynamic parts
        let mut dynamic_content = Vec::new();
        for part_fn in &self.dynamic_parts {
            dynamic_content.push(part_fn());
        }

        // Evaluate conditional parts
        for (condition, content) in &self.conditional_parts {
            if condition() {
                dynamic_content.push(content.clone());
            }
        }

        // Append dynamic content
        if !dynamic_content.is_empty() {
            let existing = static_ctx.claude_md.clone();
            let combined = if existing.is_empty() {
                dynamic_content.join("\n\n")
            } else {
                format!("{}\n\n{}", existing, dynamic_content.join("\n\n"))
            };
            static_ctx = static_ctx.with_claude_md(combined);
        }

        // Create orchestrator
        let mut orchestrator = ContextOrchestrator::new(static_ctx, &self.model);
        orchestrator.add_skill_indices(self.skill_indices);
        orchestrator.add_rule_indices(self.rule_indices);

        Ok(orchestrator)
    }

    /// Build skill summary from indices
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

    /// Load context from a .claude directory
    async fn load_file_source(
        &self,
        static_ctx: &mut StaticContext,
        base_path: &Path,
    ) -> ContextResult<()> {
        let claude_md_path = base_path.join("CLAUDE.md");
        if claude_md_path.exists() {
            let content = tokio::fs::read_to_string(&claude_md_path)
                .await
                .map_err(|e| ContextError::Source {
                    message: format!("Failed to read CLAUDE.md: {}", e),
                })?;

            if static_ctx.claude_md.is_empty() {
                static_ctx.claude_md = content;
            } else {
                static_ctx.claude_md = format!("{}\n\n{}", static_ctx.claude_md, content);
            }
        }

        Ok(())
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
            .build_sync()
            .unwrap();

        let ctx = orchestrator.static_context();
        assert!(ctx.system_prompt.contains("helpful"));
        assert!(ctx.claude_md.contains("test project"));
    }

    #[test]
    fn test_context_builder_with_skills() {
        let skill = SkillIndex::new("test", "A test skill");

        let orchestrator = ContextBuilder::new()
            .with_skill(skill)
            .build_sync()
            .unwrap();

        assert!(!orchestrator.static_context().skill_index_summary.is_empty());
    }

    #[test]
    fn test_context_builder_dynamic() {
        let orchestrator = ContextBuilder::new()
            .with_dynamic(|| format!("Dynamic value: {}", 42))
            .build_sync()
            .unwrap();

        assert!(orchestrator.static_context().claude_md.contains("42"));
    }

    #[test]
    fn test_context_builder_conditional() {
        let orchestrator = ContextBuilder::new()
            .when(|| true, "Included content")
            .when(|| false, "Excluded content")
            .build_sync()
            .unwrap();

        let md = &orchestrator.static_context().claude_md;
        assert!(md.contains("Included"));
        assert!(!md.contains("Excluded"));
    }
}
