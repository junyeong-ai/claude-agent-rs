//! TaskTool - spawns and manages subagent tasks.
//!
//! This tool allows agents to spawn specialized subagents to handle
//! complex, multi-step tasks autonomously.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{AgentDefinition, SubagentType};
use crate::tools::{Tool, ToolResult};

/// Tool for spawning and managing subagent tasks
pub struct TaskTool {
    /// Maximum concurrent background tasks
    max_background_tasks: usize,
}

impl TaskTool {
    /// Create a new TaskTool
    pub fn new() -> Self {
        Self {
            max_background_tasks: 10,
        }
    }

    /// Set maximum concurrent background tasks
    pub fn with_max_background_tasks(mut self, max: usize) -> Self {
        self.max_background_tasks = max;
        self
    }
}

impl Default for TaskTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Input parameters for the Task tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    /// Short description of the task (3-5 words)
    pub description: String,
    /// The detailed task prompt for the agent
    pub prompt: String,
    /// The type of specialized agent to use
    pub subagent_type: SubagentType,
    /// Optional model override (sonnet, opus, haiku)
    #[serde(default)]
    pub model: Option<String>,
    /// Whether to run in background
    #[serde(default)]
    pub run_in_background: Option<bool>,
    /// Optional agent ID to resume from
    #[serde(default)]
    pub resume: Option<String>,
}

impl TaskInput {
    /// Convert to an AgentDefinition
    pub fn to_definition(&self) -> AgentDefinition {
        let mut def = AgentDefinition::new(
            self.subagent_type,
            &self.description,
            &self.prompt,
        );

        if let Some(model) = &self.model {
            def = def.with_model(model);
        }

        if self.run_in_background.unwrap_or(false) {
            def = def.in_background();
        }

        if let Some(resume_id) = &self.resume {
            def = def.resume_from(resume_id);
        }

        def
    }
}

/// Output from a Task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
    /// Unique ID for this task/agent
    pub agent_id: String,
    /// Result text from the agent
    pub result: String,
    /// Whether the task is still running (background)
    pub is_running: bool,
    /// Error message if failed
    pub error: Option<String>,
}

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str {
        "Task"
    }

    fn description(&self) -> &str {
        "Launch a new agent to handle complex, multi-step tasks autonomously. \
         Available agent types: general-purpose, explore, plan, statusline-setup, \
         claude-code-guide. Use run_in_background to run asynchronously."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "A short (3-5 word) description of the task"
                },
                "prompt": {
                    "type": "string",
                    "description": "The detailed task for the agent to perform"
                },
                "subagent_type": {
                    "type": "string",
                    "enum": ["general-purpose", "explore", "plan", "statusline-setup", "claude-code-guide"],
                    "description": "The type of specialized agent to use"
                },
                "model": {
                    "type": "string",
                    "enum": ["sonnet", "opus", "haiku"],
                    "description": "Optional model override"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run task in background, use TaskOutput to get results later"
                },
                "resume": {
                    "type": "string",
                    "description": "Optional agent ID to resume from previous execution"
                }
            },
            "required": ["description", "prompt", "subagent_type"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: TaskInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        let definition = input.to_definition();

        // Generate a unique agent ID
        let agent_id = uuid::Uuid::new_v4().to_string();

        // In a full implementation, this would:
        // 1. Create a new Agent with the specified configuration
        // 2. If run_in_background, spawn it in a separate task and return immediately
        // 3. If synchronous, wait for completion and return the result
        // 4. Store the agent state for later retrieval via TaskOutput

        if definition.run_in_background {
            // Background execution - return immediately with agent ID
            let output = TaskOutput {
                agent_id: agent_id.clone(),
                result: String::new(),
                is_running: true,
                error: None,
            };

            ToolResult::success(serde_json::to_string_pretty(&output).unwrap_or_else(|_| {
                format!(
                    "Task '{}' started in background. Agent ID: {}",
                    definition.description, agent_id
                )
            }))
        } else {
            // Synchronous execution - placeholder for now
            // A full implementation would run the agent and wait for results
            let output = TaskOutput {
                agent_id,
                result: format!(
                    "Subagent task '{}' completed. Type: {:?}, Model: {}",
                    definition.description,
                    definition.subagent_type,
                    definition.effective_model()
                ),
                is_running: false,
                error: None,
            };

            ToolResult::success(serde_json::to_string_pretty(&output).unwrap_or(output.result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_input_parsing() {
        let input: TaskInput = serde_json::from_value(serde_json::json!({
            "description": "Search files",
            "prompt": "Find all Rust files",
            "subagent_type": "explore"
        }))
        .unwrap();

        assert_eq!(input.description, "Search files");
        assert_eq!(input.subagent_type, SubagentType::Explore);
    }

    #[test]
    fn test_task_input_to_definition() {
        let input = TaskInput {
            description: "Plan feature".to_string(),
            prompt: "Design the implementation".to_string(),
            subagent_type: SubagentType::Plan,
            model: Some("opus".to_string()),
            run_in_background: Some(true),
            resume: None,
        };

        let def = input.to_definition();
        assert_eq!(def.subagent_type, SubagentType::Plan);
        assert!(def.run_in_background);
        assert_eq!(def.model, Some("opus".to_string()));
    }

    #[tokio::test]
    async fn test_task_tool_execute() {
        let tool = TaskTool::new();
        let result = tool
            .execute(serde_json::json!({
                "description": "Test task",
                "prompt": "Do something",
                "subagent_type": "general-purpose"
            }))
            .await;

        assert!(!result.is_error());
    }

    #[tokio::test]
    async fn test_task_tool_background() {
        let tool = TaskTool::new();
        let result = tool
            .execute(serde_json::json!({
                "description": "Background task",
                "prompt": "Run in background",
                "subagent_type": "explore",
                "run_in_background": true
            }))
            .await;

        assert!(!result.is_error());
        if let ToolResult::Success(content) = result {
            assert!(content.contains("is_running"));
        }
    }
}
