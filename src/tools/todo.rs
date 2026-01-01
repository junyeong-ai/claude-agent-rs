//! TodoWrite tool - task tracking.

use std::sync::Mutex;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{ToolResult, TypedTool};

/// A todo item.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TodoItem {
    /// Task description (imperative form).
    pub content: String,
    /// Task status.
    pub status: TodoStatus,
    /// Task description (present continuous form).
    #[serde(rename = "activeForm")]
    pub active_form: String,
}

/// Todo status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    /// Not started.
    Pending,
    /// Currently working on.
    InProgress,
    /// Done.
    Completed,
}

/// Input for the TodoWrite tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TodoWriteInput {
    /// The updated todo list.
    pub todos: Vec<TodoItem>,
}

/// Tool for managing a task list.
pub struct TodoWriteTool {
    todos: Mutex<Vec<TodoItem>>,
}

impl TodoWriteTool {
    /// Create a new TodoWrite tool.
    pub fn new() -> Self {
        Self {
            todos: Mutex::new(Vec::new()),
        }
    }

    /// Get current todos.
    pub fn get_todos(&self) -> Vec<TodoItem> {
        self.todos.lock().unwrap().clone()
    }
}

impl Default for TodoWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for TodoWriteTool {
    type Input = TodoWriteInput;

    const NAME: &'static str = "TodoWrite";
    const DESCRIPTION: &'static str = "Create and manage a structured task list. Use for multi-step tasks, \
        complex work that benefits from tracking, or when explicitly requested. \
        Each task needs 'content' (imperative form like 'Fix bug') and \
        'activeForm' (present continuous like 'Fixing bug').";

    async fn handle(&self, input: TodoWriteInput) -> ToolResult {
        let in_progress_count = input
            .todos
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();

        if in_progress_count > 1 {
            return ToolResult::error(
                "Only one task can be in_progress at a time. Complete the current task first.",
            );
        }

        *self.todos.lock().unwrap() = input.todos.clone();

        let mut response = String::from("Todo list updated:\n");
        for (i, todo) in input.todos.iter().enumerate() {
            let status_icon = match todo.status {
                TodoStatus::Pending => "○",
                TodoStatus::InProgress => "◐",
                TodoStatus::Completed => "●",
            };
            response.push_str(&format!("{}. {} {}\n", i + 1, status_icon, todo.content));
        }

        ToolResult::success(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[tokio::test]
    async fn test_todo_write() {
        let tool = TodoWriteTool::new();
        let result = tool
            .execute(serde_json::json!({
                "todos": [
                    {
                        "content": "Fix the bug",
                        "status": "in_progress",
                        "activeForm": "Fixing the bug"
                    },
                    {
                        "content": "Write tests",
                        "status": "pending",
                        "activeForm": "Writing tests"
                    }
                ]
            }))
            .await;

        assert!(!result.is_error());
        assert_eq!(tool.get_todos().len(), 2);
    }

    #[tokio::test]
    async fn test_todo_multiple_in_progress() {
        let tool = TodoWriteTool::new();
        let result = tool
            .execute(serde_json::json!({
                "todos": [
                    {
                        "content": "Task 1",
                        "status": "in_progress",
                        "activeForm": "Doing task 1"
                    },
                    {
                        "content": "Task 2",
                        "status": "in_progress",
                        "activeForm": "Doing task 2"
                    }
                ]
            }))
            .await;

        assert!(result.is_error());
    }
}
