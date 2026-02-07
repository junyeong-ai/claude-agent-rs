//! TaskOutputTool - retrieves results from running or completed tasks.

use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::task_registry::TaskRegistry;
use crate::session::SessionState;
use crate::tools::{ExecutionContext, SchemaTool};
use crate::types::ToolResult;

#[derive(Clone)]
pub struct TaskOutputTool {
    registry: TaskRegistry,
}

impl TaskOutputTool {
    pub fn new(registry: TaskRegistry) -> Self {
        Self { registry }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct TaskOutputInput {
    /// The task ID to get output from
    pub task_id: String,
    /// Whether to wait for completion
    #[serde(default = "default_block")]
    pub block: bool,
    /// Max wait time in ms
    #[serde(default = "default_timeout")]
    #[schemars(range(min = 0, max = 600000))]
    pub timeout: u64,
}

fn default_block() -> bool {
    true
}

fn default_timeout() -> u64 {
    30000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    NotFound,
}

impl From<SessionState> for TaskStatus {
    fn from(state: SessionState) -> Self {
        match state {
            SessionState::Created | SessionState::Active | SessionState::WaitingForTools => {
                TaskStatus::Running
            }
            SessionState::Completed => TaskStatus::Completed,
            SessionState::Failed => TaskStatus::Failed,
            SessionState::Cancelled => TaskStatus::Cancelled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutputResult {
    pub task_id: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[async_trait]
impl SchemaTool for TaskOutputTool {
    type Input = TaskOutputInput;

    const NAME: &'static str = "TaskOutput";
    const DESCRIPTION: &'static str = r#"
- Retrieves output from a running or completed task (background shell, agent, or remote session)
- Takes a task_id parameter identifying the task
- Returns the task output along with status information
- Use block=true (default) to wait for task completion
- Use block=false for non-blocking check of current status
- Task IDs can be found using the Task tool response
- Works with all task types: background shells, async agents, and remote sessions
- Output is limited to prevent excessive memory usage; for larger outputs, consider streaming
- Important: task_id is the Task tool's returned ID, NOT a process PID"#;

    async fn handle(&self, input: TaskOutputInput, _context: &ExecutionContext) -> ToolResult {
        let timeout = Duration::from_millis(input.timeout.min(600000));

        let result = if input.block {
            self.registry
                .wait_for_completion(&input.task_id, timeout)
                .await
        } else {
            self.registry.get_result(&input.task_id).await
        };

        let output = match result {
            Some((status, output, error)) => TaskOutputResult {
                task_id: input.task_id,
                status: status.into(),
                output,
                error,
            },
            None => TaskOutputResult {
                task_id: input.task_id,
                status: TaskStatus::NotFound,
                output: None,
                error: Some("Task not found".to_string()),
            },
        };

        ToolResult::success(
            serde_json::to_string_pretty(&output)
                .unwrap_or_else(|_| format!("Task status: {:?}", output.status)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentMetrics, AgentResult, AgentState};
    use crate::session::MemoryPersistence;
    use crate::tools::Tool;
    use crate::types::{StopReason, ToolOutput, Usage};
    use std::sync::Arc;

    // Use valid UUIDs for tests to ensure consistent session IDs
    const TASK_1_UUID: &str = "00000000-0000-0000-0000-000000000011";
    const TASK_2_UUID: &str = "00000000-0000-0000-0000-000000000012";
    const TASK_3_UUID: &str = "00000000-0000-0000-0000-000000000013";

    fn test_registry() -> TaskRegistry {
        TaskRegistry::new(Arc::new(MemoryPersistence::new()))
    }

    fn mock_result() -> AgentResult {
        AgentResult {
            text: "Completed successfully".to_string(),
            usage: Usage::default(),
            tool_calls: 0,
            iterations: 1,
            stop_reason: StopReason::EndTurn,
            state: AgentState::Completed,
            metrics: AgentMetrics::default(),
            session_id: "test-session".to_string(),
            structured_output: None,
            messages: Vec::new(),
            uuid: "test-uuid".to_string(),
        }
    }

    #[tokio::test]
    async fn test_task_output_completed() {
        let registry = test_registry();
        registry
            .register(TASK_1_UUID.into(), "Explore".into(), "Test".into())
            .await;
        registry.complete(TASK_1_UUID, mock_result()).await;

        let tool = TaskOutputTool::new(registry);
        let context = crate::tools::ExecutionContext::default();
        let result = tool
            .execute(
                serde_json::json!({
                    "task_id": TASK_1_UUID
                }),
                &context,
            )
            .await;

        assert!(!result.is_error());
        if let ToolOutput::Success(content) = &result.output {
            assert!(content.contains("completed"));
        }
    }

    #[tokio::test]
    async fn test_task_output_not_found() {
        let registry = test_registry();
        let tool = TaskOutputTool::new(registry);
        let context = crate::tools::ExecutionContext::default();

        let result = tool
            .execute(
                serde_json::json!({
                    "task_id": "nonexistent"
                }),
                &context,
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            assert!(content.contains("not_found"));
        }
    }

    #[tokio::test]
    async fn test_task_output_non_blocking() {
        let registry = test_registry();
        registry
            .register(TASK_2_UUID.into(), "Explore".into(), "Running".into())
            .await;

        let tool = TaskOutputTool::new(registry);
        let context = crate::tools::ExecutionContext::default();
        let result = tool
            .execute(
                serde_json::json!({
                    "task_id": TASK_2_UUID,
                    "block": false
                }),
                &context,
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            assert!(content.contains("running"));
        }
    }

    #[tokio::test]
    async fn test_task_output_failed() {
        let registry = test_registry();
        registry
            .register(TASK_3_UUID.into(), "Explore".into(), "Failing".into())
            .await;
        registry
            .fail(TASK_3_UUID, "Something went wrong".into())
            .await;

        let tool = TaskOutputTool::new(registry);
        let context = crate::tools::ExecutionContext::default();
        let result = tool
            .execute(
                serde_json::json!({
                    "task_id": TASK_3_UUID
                }),
                &context,
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            assert!(content.contains("failed"));
            assert!(content.contains("Something went wrong"));
        }
    }
}
