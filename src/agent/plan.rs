//! Plan mode tools - EnterPlanMode and ExitPlanMode.
//!
//! These tools allow agents to transition into and out of plan mode,
//! which is used for designing implementation approaches before coding.

use async_trait::async_trait;

use crate::tools::{Tool, ToolResult};

/// Tool for entering plan mode
pub struct EnterPlanModeTool;

impl EnterPlanModeTool {
    /// Create a new EnterPlanModeTool
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnterPlanModeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "EnterPlanMode"
    }

    fn description(&self) -> &str {
        "Transition into plan mode for non-trivial implementation tasks. \
         Use when: adding new features, multiple valid approaches exist, \
         code modifications affect existing behavior, architectural decisions needed, \
         multi-file changes, or unclear requirements. \
         In plan mode, explore codebase and design approach for user approval."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    }

    async fn execute(&self, _input: serde_json::Value) -> ToolResult {
        // In a full implementation, this would:
        // 1. Change the agent's mode to PlanMode
        // 2. Update the system prompt to indicate planning context
        // 3. Restrict tools to read-only operations
        // 4. Set up the plan file for writing

        ToolResult::success(
            "Entered plan mode. You can now:\n\
             - Explore the codebase using Glob, Grep, and Read tools\n\
             - Design an implementation approach\n\
             - Write your plan to a plan file\n\
             - Use AskUserQuestion if you need to clarify approaches\n\
             - Use ExitPlanMode when ready for user to review and approve\n\n\
             IMPORTANT: This requires user approval before proceeding.",
        )
    }
}

/// Tool for exiting plan mode
pub struct ExitPlanModeTool;

impl ExitPlanModeTool {
    /// Create a new ExitPlanModeTool
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExitPlanModeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "ExitPlanMode"
    }

    fn description(&self) -> &str {
        "Signal that planning is complete and ready for user approval. \
         Use ONLY when: the task requires planning implementation steps that require writing code. \
         Do NOT use for research tasks (searching, reading, understanding codebase). \
         You should have already written your plan to a plan file."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": true
        })
    }

    async fn execute(&self, _input: serde_json::Value) -> ToolResult {
        // In a full implementation, this would:
        // 1. Validate that a plan was written
        // 2. Present the plan to the user for approval
        // 3. Wait for user decision (approve/reject/modify)
        // 4. If approved, transition back to normal mode and begin implementation

        ToolResult::success(
            "Exiting plan mode. The plan is ready for user review.\n\n\
             The user will now review the plan and can:\n\
             - Approve: Continue with implementation\n\
             - Reject: Return to planning with feedback\n\
             - Modify: Request changes before approval\n\n\
             Waiting for user decision...",
        )
    }
}

/// Plan mode state
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlanModeState {
    /// Not in plan mode - normal operation
    #[default]
    Inactive,
    /// In plan mode - exploring and planning
    Planning,
    /// Plan complete - waiting for user approval
    AwaitingApproval,
    /// Plan approved - transitioning to implementation
    Approved,
    /// Plan rejected - returning to planning with feedback
    Rejected,
}

impl PlanModeState {
    /// Check if plan mode is active
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Planning | Self::AwaitingApproval)
    }

    /// Check if ready for implementation
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enter_plan_mode() {
        let tool = EnterPlanModeTool::new();
        let result = tool.execute(serde_json::json!({})).await;

        assert!(!result.is_error());
        if let ToolResult::Success(content) = result {
            assert!(content.contains("plan mode"));
        }
    }

    #[tokio::test]
    async fn test_exit_plan_mode() {
        let tool = ExitPlanModeTool::new();
        let result = tool.execute(serde_json::json!({})).await;

        assert!(!result.is_error());
        if let ToolResult::Success(content) = result {
            assert!(content.contains("user review"));
        }
    }

    #[test]
    fn test_plan_mode_state() {
        assert!(!PlanModeState::Inactive.is_active());
        assert!(PlanModeState::Planning.is_active());
        assert!(PlanModeState::AwaitingApproval.is_active());
        assert!(!PlanModeState::Approved.is_active());

        assert!(!PlanModeState::Planning.is_approved());
        assert!(PlanModeState::Approved.is_approved());
    }
}
