//! Session-related types for persistence and tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::state::SessionId;

/// Tool execution record for tracking all tool invocations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolExecution {
    pub id: Uuid,
    pub session_id: SessionId,
    pub message_id: Option<String>,

    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_output: String,
    pub is_error: bool,
    pub error_message: Option<String>,

    pub duration_ms: u64,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,

    pub plan_id: Option<Uuid>,
    pub spawned_session_id: Option<SessionId>,

    pub created_at: DateTime<Utc>,
}

impl ToolExecution {
    pub fn new(
        session_id: SessionId,
        tool_name: impl Into<String>,
        tool_input: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            message_id: None,
            tool_name: tool_name.into(),
            tool_input,
            tool_output: String::new(),
            is_error: false,
            error_message: None,
            duration_ms: 0,
            input_tokens: None,
            output_tokens: None,
            plan_id: None,
            spawned_session_id: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_output(mut self, output: impl Into<String>, is_error: bool) -> Self {
        self.tool_output = output.into();
        self.is_error = is_error;
        self
    }

    pub fn with_error(mut self, message: impl Into<String>) -> Self {
        self.is_error = true;
        self.error_message = Some(message.into());
        self
    }

    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn with_plan(mut self, plan_id: Uuid) -> Self {
        self.plan_id = Some(plan_id);
        self
    }

    pub fn with_spawned_session(mut self, session_id: SessionId) -> Self {
        self.spawned_session_id = Some(session_id);
        self
    }

    pub fn with_message(mut self, message_id: impl Into<String>) -> Self {
        self.message_id = Some(message_id.into());
        self
    }

    pub fn with_tokens(mut self, input: u32, output: u32) -> Self {
        self.input_tokens = Some(input);
        self.output_tokens = Some(output);
        self
    }
}

/// Plan status in the planning workflow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    #[default]
    Draft,
    Approved,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

impl PlanStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_execute(&self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// Plan record for tracking planning workflow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: Uuid,
    pub session_id: SessionId,

    pub name: Option<String>,
    pub content: String,
    pub status: PlanStatus,

    pub created_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    pub error: Option<String>,
}

impl Plan {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            name: None,
            content: String::new(),
            status: PlanStatus::Draft,
            created_at: Utc::now(),
            approved_at: None,
            started_at: None,
            completed_at: None,
            error: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn approve(&mut self) {
        self.status = PlanStatus::Approved;
        self.approved_at = Some(Utc::now());
    }

    pub fn start_execution(&mut self) {
        self.status = PlanStatus::Executing;
        self.started_at = Some(Utc::now());
    }

    pub fn complete(&mut self) {
        self.status = PlanStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = PlanStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error.into());
    }

    pub fn cancel(&mut self) {
        self.status = PlanStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }
}

/// Todo item status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

/// Todo item for task tracking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: Uuid,
    pub session_id: SessionId,

    pub content: String,
    pub active_form: String,
    pub status: TodoStatus,

    pub plan_id: Option<Uuid>,

    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl TodoItem {
    pub fn new(
        session_id: SessionId,
        content: impl Into<String>,
        active_form: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            content: content.into(),
            active_form: active_form.into(),
            status: TodoStatus::Pending,
            plan_id: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn with_plan(mut self, plan_id: Uuid) -> Self {
        self.plan_id = Some(plan_id);
        self
    }

    pub fn start(&mut self) {
        self.status = TodoStatus::InProgress;
        self.started_at = Some(Utc::now());
    }

    pub fn complete(&mut self) {
        self.status = TodoStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    pub fn status_icon(&self) -> &'static str {
        match self.status {
            TodoStatus::Pending => "○",
            TodoStatus::InProgress => "◐",
            TodoStatus::Completed => "●",
        }
    }
}

/// Compact history record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactRecord {
    pub id: Uuid,
    pub session_id: SessionId,

    pub summary: String,
    pub original_count: usize,
    pub new_count: usize,
    pub saved_tokens: usize,
    pub instructions: Option<String>,

    pub created_at: DateTime<Utc>,
}

impl CompactRecord {
    pub fn new(
        session_id: SessionId,
        summary: impl Into<String>,
        original_count: usize,
        new_count: usize,
        saved_tokens: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            summary: summary.into(),
            original_count,
            new_count,
            saved_tokens,
            instructions: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }
}

/// Filter for querying tool executions.
#[derive(Clone, Debug, Default)]
pub struct ToolExecutionFilter {
    pub tool_name: Option<String>,
    pub plan_id: Option<Uuid>,
    pub is_error: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl ToolExecutionFilter {
    pub fn by_tool(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: Some(tool_name.into()),
            ..Default::default()
        }
    }

    pub fn by_plan(plan_id: Uuid) -> Self {
        Self {
            plan_id: Some(plan_id),
            ..Default::default()
        }
    }

    pub fn errors_only() -> Self {
        Self {
            is_error: Some(true),
            ..Default::default()
        }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Aggregated statistics for a session.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_messages: usize,
    pub total_tool_calls: usize,
    pub tool_success_count: usize,
    pub tool_error_count: usize,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub avg_tool_duration_ms: f64,
    pub plans_count: usize,
    pub todos_completed: usize,
    pub todos_total: usize,
    pub compacts_count: usize,
    pub subagent_count: usize,
}

impl SessionStats {
    pub fn tool_success_rate(&self) -> f64 {
        if self.total_tool_calls == 0 {
            1.0
        } else {
            self.tool_success_count as f64 / self.total_tool_calls as f64
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }
}

/// Session tree for hierarchical view with subagents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionTree {
    pub session_id: SessionId,
    pub session_type: super::state::SessionType,
    pub stats: SessionStats,
    pub children: Vec<SessionTree>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_execution_builder() {
        let session_id = SessionId::new();
        let exec = ToolExecution::new(session_id, "Bash", serde_json::json!({"command": "ls"}))
            .with_output("file1\nfile2", false)
            .with_duration(150);

        assert_eq!(exec.tool_name, "Bash");
        assert_eq!(exec.duration_ms, 150);
        assert!(!exec.is_error);
    }

    #[test]
    fn test_plan_lifecycle() {
        let session_id = SessionId::new();
        let mut plan = Plan::new(session_id)
            .with_name("Implement auth")
            .with_content("1. Create user model\n2. Add endpoints");

        assert_eq!(plan.status, PlanStatus::Draft);

        plan.approve();
        assert_eq!(plan.status, PlanStatus::Approved);
        assert!(plan.approved_at.is_some());

        plan.start_execution();
        assert_eq!(plan.status, PlanStatus::Executing);

        plan.complete();
        assert_eq!(plan.status, PlanStatus::Completed);
        assert!(plan.status.is_terminal());
    }

    #[test]
    fn test_todo_item() {
        let session_id = SessionId::new();
        let mut todo = TodoItem::new(session_id, "Fix bug", "Fixing bug");

        assert_eq!(todo.status, TodoStatus::Pending);
        assert_eq!(todo.status_icon(), "○");

        todo.start();
        assert_eq!(todo.status, TodoStatus::InProgress);
        assert_eq!(todo.status_icon(), "◐");

        todo.complete();
        assert_eq!(todo.status, TodoStatus::Completed);
        assert_eq!(todo.status_icon(), "●");
    }

    #[test]
    fn test_session_stats() {
        let stats = SessionStats {
            total_tool_calls: 10,
            tool_success_count: 8,
            tool_error_count: 2,
            total_input_tokens: 1000,
            total_output_tokens: 500,
            ..Default::default()
        };

        assert!((stats.tool_success_rate() - 0.8).abs() < 0.001);
        assert_eq!(stats.total_tokens(), 1500);
    }
}
