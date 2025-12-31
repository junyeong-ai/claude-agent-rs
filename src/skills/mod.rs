//! Skill system - specialized workflows and reusable patterns.
//!
//! This module provides the skill system that allows developers to register,
//! discover, and execute skills (specialized workflows), as well as slash
//! commands for user-defined shortcuts.

mod commands;
mod executor;
mod loader;
mod registry;
mod skill_tool;

pub use commands::{CommandLoader, SlashCommand};
pub use executor::{ExecutionMode, SkillExecutionCallback, SkillExecutor};
pub use loader::{SkillLoader, SkillSource};
pub use registry::SkillRegistry;
pub use skill_tool::SkillTool;

use serde::{Deserialize, Serialize};

/// Source type for a skill
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillSourceType {
    /// Built-in skill (part of the SDK)
    Builtin,
    /// User-defined skill (from user config)
    #[default]
    User,
    /// Project-defined skill (from project directory)
    Project,
    /// Managed skill (from managed location)
    Managed,
}

/// Definition of a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    /// Unique name of the skill
    pub name: String,
    /// Short description of what the skill does
    pub description: String,
    /// The skill prompt/template content
    pub content: String,
    /// Source type of the skill
    #[serde(default)]
    pub source_type: SkillSourceType,
    /// Location where the skill was loaded from
    #[serde(default)]
    pub location: Option<String>,
    /// Optional trigger patterns that activate this skill
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Optional arguments schema
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
    /// Allowed tools for this skill (security boundary)
    #[serde(default, alias = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    /// Model override for this skill (cost optimization)
    #[serde(default)]
    pub model: Option<String>,
}

impl SkillDefinition {
    /// Create a new skill definition
    pub fn new(name: impl Into<String>, description: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            content: content.into(),
            source_type: SkillSourceType::User,
            location: None,
            triggers: Vec::new(),
            arguments: None,
            allowed_tools: Vec::new(),
            model: None,
        }
    }

    /// Set the source type
    pub fn with_source_type(mut self, source_type: SkillSourceType) -> Self {
        self.source_type = source_type;
        self
    }

    /// Set the location
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Add a trigger pattern
    pub fn with_trigger(mut self, trigger: impl Into<String>) -> Self {
        self.triggers.push(trigger.into());
        self
    }

    /// Set arguments schema
    pub fn with_arguments(mut self, schema: serde_json::Value) -> Self {
        self.arguments = Some(schema);
        self
    }

    /// Set allowed tools for this skill (security boundary)
    pub fn with_allowed_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.allowed_tools = tools.into_iter().map(Into::into).collect();
        self
    }

    /// Add a single allowed tool
    pub fn with_allowed_tool(mut self, tool: impl Into<String>) -> Self {
        self.allowed_tools.push(tool.into());
        self
    }

    /// Set model override for cost optimization
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Get the full qualified name (for namespaced skills)
    pub fn qualified_name(&self) -> &str {
        &self.name
    }

    /// Check if this skill has tool restrictions
    pub fn has_tool_restrictions(&self) -> bool {
        !self.allowed_tools.is_empty()
    }

    /// Check if a tool is allowed for this skill
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        if self.allowed_tools.is_empty() {
            return true; // No restrictions
        }
        self.allowed_tools.iter().any(|t| {
            // Support pattern matching: "Bash(git:*)" matches "Bash"
            if let Some(base) = t.split('(').next() {
                base == tool_name || t == tool_name
            } else {
                t == tool_name
            }
        })
    }

    /// Check if this skill matches a trigger
    pub fn matches_trigger(&self, input: &str) -> bool {
        // Check if any trigger pattern matches
        self.triggers.iter().any(|t| {
            input.to_lowercase().contains(&t.to_lowercase())
        })
    }
}

/// Result of skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    /// Whether execution was successful
    pub success: bool,
    /// Output from the skill
    pub output: String,
    /// Any error message
    pub error: Option<String>,
    /// Allowed tools for this execution (security boundary)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
    /// Model to use for this execution (cost optimization)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl SkillResult {
    /// Create a successful result
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
            allowed_tools: Vec::new(),
            model: None,
        }
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(message.into()),
            allowed_tools: Vec::new(),
            model: None,
        }
    }

    /// Set allowed tools for this result
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    /// Set model for this result
    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    /// Check if this result has tool restrictions
    pub fn has_tool_restrictions(&self) -> bool {
        !self.allowed_tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_definition() {
        let skill = SkillDefinition::new(
            "commit",
            "Create a git commit",
            "Analyze changes and create commit message",
        )
        .with_source_type(SkillSourceType::Builtin)
        .with_trigger("/commit");

        assert_eq!(skill.name, "commit");
        assert_eq!(skill.source_type, SkillSourceType::Builtin);
        assert!(skill.matches_trigger("/commit please"));
    }

    #[test]
    fn test_skill_result() {
        let success = SkillResult::success("Done!");
        assert!(success.success);
        assert!(success.error.is_none());

        let error = SkillResult::error("Failed");
        assert!(!error.success);
        assert!(error.error.is_some());
    }

    #[test]
    fn test_skill_allowed_tools() {
        let skill = SkillDefinition::new("reader", "Read files", "Read: $ARGUMENTS")
            .with_allowed_tools(["Read", "Grep", "Glob"]);

        assert!(skill.has_tool_restrictions());
        assert!(skill.is_tool_allowed("Read"));
        assert!(skill.is_tool_allowed("Grep"));
        assert!(!skill.is_tool_allowed("Bash"));
        assert!(!skill.is_tool_allowed("Write"));
    }

    #[test]
    fn test_skill_allowed_tools_pattern() {
        let skill = SkillDefinition::new("git-helper", "Git commands", "Git: $ARGUMENTS")
            .with_allowed_tools(["Bash(git:*)", "Read"]);

        assert!(skill.is_tool_allowed("Bash"));  // Base tool name
        assert!(skill.is_tool_allowed("Read"));
        assert!(!skill.is_tool_allowed("Write"));
    }

    #[test]
    fn test_skill_no_restrictions() {
        let skill = SkillDefinition::new("any", "Any tools", "Do: $ARGUMENTS");

        assert!(!skill.has_tool_restrictions());
        assert!(skill.is_tool_allowed("Bash"));
        assert!(skill.is_tool_allowed("Read"));
        assert!(skill.is_tool_allowed("Anything"));
    }

    #[test]
    fn test_skill_model_override() {
        let skill = SkillDefinition::new("fast-task", "Quick task", "Do: $ARGUMENTS")
            .with_model("claude-haiku-4-5-20251001");

        assert_eq!(skill.model, Some("claude-haiku-4-5-20251001".to_string()));
    }

    #[test]
    fn test_skill_result_with_context() {
        let result = SkillResult::success("Output")
            .with_allowed_tools(vec!["Read".to_string(), "Grep".to_string()])
            .with_model(Some("claude-haiku-4-5-20251001".to_string()));

        assert!(result.has_tool_restrictions());
        assert_eq!(result.allowed_tools, vec!["Read", "Grep"]);
        assert_eq!(result.model, Some("claude-haiku-4-5-20251001".to_string()));
    }
}
