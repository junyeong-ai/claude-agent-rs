//! Skill Index Types for Progressive Disclosure
//!
//! SkillIndex contains minimal metadata (name + description) that is always loaded,
//! while the full skill body is loaded only when the skill is activated.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Skill index entry - minimal metadata always loaded in context
///
/// This is used for skill discovery and routing. The full skill body
/// is only loaded when explicitly activated.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillIndex {
    /// Skill name (unique identifier)
    pub name: String,

    /// Skill description - used by Claude for semantic matching
    pub description: String,

    /// Trigger keywords for fast matching (optional)
    #[serde(default)]
    pub triggers: Vec<String>,

    /// Allowed tools for this skill (if restricted)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,

    /// Source location for loading full skill
    pub source: SkillSource,

    /// Skill scope (priority order)
    pub scope: SkillScope,
}

/// Source location for loading the full skill definition
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillSource {
    /// File system path (CLI compatible)
    File {
        /// Path to the SKILL.md file
        path: PathBuf,
    },

    /// Database storage (server environment)
    Database {
        /// Skill ID in database
        skill_id: String,
    },

    /// HTTP endpoint (remote skill)
    Http {
        /// URL to fetch skill definition
        url: String,
    },

    /// In-memory (already loaded, code-defined)
    InMemory,
}

/// Skill scope determines loading priority
///
/// Higher priority scopes override lower priority when names conflict.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    /// Plugin skills (lowest priority)
    Plugin = 0,
    /// Project skills (.claude/skills/)
    #[default]
    Project = 1,
    /// User global skills (~/.claude/skills/)
    User = 2,
    /// Enterprise skills (highest priority)
    Enterprise = 3,
}

impl SkillIndex {
    /// Create a new skill index entry
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            triggers: Vec::new(),
            allowed_tools: None,
            source: SkillSource::InMemory,
            scope: SkillScope::default(),
        }
    }

    /// Add trigger keywords
    pub fn with_triggers(mut self, triggers: Vec<String>) -> Self {
        self.triggers = triggers;
        self
    }

    /// Set allowed tools
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    /// Set source location
    pub fn with_source(mut self, source: SkillSource) -> Self {
        self.source = source;
        self
    }

    /// Set scope
    pub fn with_scope(mut self, scope: SkillScope) -> Self {
        self.scope = scope;
        self
    }

    /// Check if the skill matches any trigger keywords
    pub fn matches_triggers(&self, input: &str) -> bool {
        let input_lower = input.to_lowercase();
        self.triggers
            .iter()
            .any(|trigger| input_lower.contains(&trigger.to_lowercase()))
    }

    /// Check if this is a slash command match (e.g., /skill-name)
    pub fn matches_command(&self, input: &str) -> bool {
        if let Some(cmd) = input.strip_prefix('/') {
            let cmd_lower = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
            self.name.to_lowercase() == cmd_lower
        } else {
            false
        }
    }

    /// Generate a summary line for context injection
    ///
    /// Format: `- skill-name: description (scope: project)`
    pub fn to_summary_line(&self) -> String {
        format!(
            "- {}: {} (scope: {:?})",
            self.name, self.description, self.scope
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_index_creation() {
        let skill = SkillIndex::new("commit", "Create a git commit with conventional format")
            .with_triggers(vec!["git commit".into(), "commit changes".into()])
            .with_scope(SkillScope::User);

        assert_eq!(skill.name, "commit");
        assert!(skill.matches_triggers("I want to git commit these changes"));
        assert!(!skill.matches_triggers("deploy the application"));
    }

    #[test]
    fn test_command_matching() {
        let skill = SkillIndex::new("commit", "Create a git commit");

        assert!(skill.matches_command("/commit"));
        assert!(skill.matches_command("/commit -m 'message'"));
        assert!(!skill.matches_command("/other"));
        assert!(!skill.matches_command("commit"));
    }

    #[test]
    fn test_scope_ordering() {
        assert!(SkillScope::Enterprise > SkillScope::User);
        assert!(SkillScope::User > SkillScope::Project);
        assert!(SkillScope::Project > SkillScope::Plugin);
    }

    #[test]
    fn test_summary_line() {
        let skill = SkillIndex::new("test", "A test skill").with_scope(SkillScope::Project);
        let summary = skill.to_summary_line();
        assert!(summary.contains("test"));
        assert!(summary.contains("A test skill"));
    }
}
