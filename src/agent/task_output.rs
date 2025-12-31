//! TaskOutputTool - retrieves results from running or completed tasks.
//!
//! This tool allows agents to check on the status and retrieve results
//! from background tasks, shell commands, and remote sessions.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::tools::{Tool, ToolResult};

/// Tool for retrieving output from running or completed tasks
pub struct TaskOutputTool;

impl TaskOutputTool {
    /// Create a new TaskOutputTool
    pub fn new() -> Self {
        Self
    }
}

impl Default for TaskOutputTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Input parameters for the TaskOutput tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutputInput {
    /// The task ID to get output from
    pub task_id: String,
    /// Whether to wait for task completion (default: true)
    #[serde(default = "default_block")]
    pub block: bool,
    /// Maximum wait time in milliseconds (default: 30000, max: 600000)
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_block() -> bool {
    true
}

fn default_timeout() -> u64 {
    30000
}

/// Status of a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is still running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with an error
    Failed,
    /// Task was cancelled
    Cancelled,
    /// Task not found
    NotFound,
}

/// Output from TaskOutput tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutputResult {
    /// The task ID
    pub task_id: String,
    /// Current status of the task
    pub status: TaskStatus,
    /// Output content (if available)
    pub output: Option<String>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Exit code (for shell tasks)
    pub exit_code: Option<i32>,
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "TaskOutput"
    }

    fn description(&self) -> &str {
        "Retrieves output from a running or completed task (background shell, agent, or remote session). \
         Use block=true (default) to wait for completion. Use block=false for non-blocking check. \
         Task IDs can be found using the /tasks command."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to get output from"
                },
                "block": {
                    "type": "boolean",
                    "description": "Whether to wait for completion (default: true)",
                    "default": true
                },
                "timeout": {
                    "type": "number",
                    "description": "Max wait time in ms (default: 30000, max: 600000)",
                    "default": 30000,
                    "minimum": 0,
                    "maximum": 600000
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: TaskOutputInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // Validate timeout
        let timeout = input.timeout.min(600000);

        // In a full implementation, this would:
        // 1. Look up the task by ID in a task registry
        // 2. If block=true and task is running, wait up to timeout
        // 3. Return the task status and any available output

        // For now, return a placeholder response indicating task not found
        // A TaskRegistry would be needed to track running tasks
        let result = TaskOutputResult {
            task_id: input.task_id.clone(),
            status: TaskStatus::NotFound,
            output: None,
            error: Some(format!(
                "Task '{}' not found. Task tracking is not yet fully implemented. \
                 Timeout was set to {}ms, blocking={}",
                input.task_id, timeout, input.block
            )),
            exit_code: None,
        };

        ToolResult::success(
            serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| format!("Task '{}' not found", input.task_id)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_output_input_defaults() {
        let input: TaskOutputInput = serde_json::from_value(serde_json::json!({
            "task_id": "test_123"
        }))
        .unwrap();

        assert_eq!(input.task_id, "test_123");
        assert!(input.block);
        assert_eq!(input.timeout, 30000);
    }

    #[test]
    fn test_task_output_input_custom() {
        let input: TaskOutputInput = serde_json::from_value(serde_json::json!({
            "task_id": "test_456",
            "block": false,
            "timeout": 5000
        }))
        .unwrap();

        assert!(!input.block);
        assert_eq!(input.timeout, 5000);
    }

    #[tokio::test]
    async fn test_task_output_not_found() {
        let tool = TaskOutputTool::new();
        let result = tool
            .execute(serde_json::json!({
                "task_id": "nonexistent_task"
            }))
            .await;

        // Should return success with NotFound status
        assert!(!result.is_error());
        if let ToolResult::Success(content) = result {
            assert!(content.contains("not_found") || content.contains("NotFound"));
        }
    }
}
