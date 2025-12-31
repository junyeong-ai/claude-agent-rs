//! SkillTool - tool wrapper for skill execution.
//!
//! This tool allows invoking skills from within the agent tool system.

use std::sync::Arc;
use tokio::sync::RwLock;

use async_trait::async_trait;
use serde::Deserialize;

use super::SkillExecutor;
use crate::tools::{Tool, ToolResult};

/// Tool for executing skills
pub struct SkillTool {
    executor: Arc<RwLock<SkillExecutor>>,
}

impl SkillTool {
    /// Create a new SkillTool with the given executor
    pub fn new(executor: SkillExecutor) -> Self {
        Self {
            executor: Arc::new(RwLock::new(executor)),
        }
    }

    /// Create a SkillTool with default skills
    pub fn with_defaults() -> Self {
        Self::new(SkillExecutor::with_defaults())
    }

    /// Get a reference to the executor
    pub fn executor(&self) -> &Arc<RwLock<SkillExecutor>> {
        &self.executor
    }
}

impl Default for SkillTool {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// Input for the Skill tool
#[derive(Debug, Clone, Deserialize)]
pub struct SkillInput {
    /// The skill name (e.g., "commit", "review-pr")
    pub skill: String,
    /// Optional arguments for the skill
    #[serde(default)]
    pub args: Option<String>,
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "Skill"
    }

    fn description(&self) -> &str {
        "Execute a skill within the main conversation. Skills provide specialized capabilities \
         and domain knowledge. Use skill name like \"commit\" or fully qualified name like \
         \"speckit:plan\". Check available skills before invoking."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The skill name (e.g., \"commit\", \"review-pr\", or \"namespace:skill\")"
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments for the skill"
                }
            },
            "required": ["skill"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: SkillInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        let executor = self.executor.read().await;
        let result = executor.execute(&input.skill, input.args.as_deref()).await;

        if result.success {
            ToolResult::success(result.output)
        } else {
            ToolResult::error(
                result
                    .error
                    .unwrap_or_else(|| "Skill execution failed".to_string()),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{SkillDefinition, SkillRegistry};

    #[tokio::test]
    async fn test_skill_tool_execute() {
        let mut registry = SkillRegistry::new();
        registry.register(SkillDefinition::new(
            "test",
            "Test skill",
            "Execute with: $ARGUMENTS",
        ));

        let executor = SkillExecutor::new(registry);
        let tool = SkillTool::new(executor);

        let result = tool
            .execute(serde_json::json!({
                "skill": "test",
                "args": "my args"
            }))
            .await;

        assert!(!result.is_error());
        if let ToolResult::Success(content) = result {
            assert!(content.contains("my args"));
        }
    }

    #[tokio::test]
    async fn test_skill_tool_not_found() {
        let tool = SkillTool::new(SkillExecutor::new(SkillRegistry::new()));

        let result = tool
            .execute(serde_json::json!({
                "skill": "nonexistent"
            }))
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_skill_tool_with_defaults() {
        let tool = SkillTool::with_defaults();

        // Should have the commit skill by default
        let executor = tool.executor.read().await;
        assert!(executor.has_skill("commit"));
    }
}
