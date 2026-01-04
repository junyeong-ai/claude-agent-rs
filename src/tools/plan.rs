//! Plan tool for structured planning workflow.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::SchemaTool;
use super::context::ExecutionContext;
use crate::session::session_state::ToolState;
use crate::types::ToolResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlanAction {
    Start,
    Complete,
    Cancel,
    Update,
    Status,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct PlanInput {
    /// Action: "start", "complete", "cancel", "update", or "status"
    pub action: PlanAction,
    /// Plan name (optional, used with "start")
    #[serde(default)]
    pub name: Option<String>,
    /// Plan content (optional, used with "update")
    #[serde(default)]
    pub content: Option<String>,
}

pub struct PlanTool {
    state: ToolState,
}

impl PlanTool {
    pub fn new(state: ToolState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl SchemaTool for PlanTool {
    type Input = PlanInput;

    const NAME: &'static str = "Plan";
    const DESCRIPTION: &'static str = r#"Manage structured planning workflow for complex implementation tasks.

## Actions

- **start**: Begin plan mode for complex tasks (creates Draft plan)
- **complete**: Finalize plan and proceed to implementation (approves plan)
- **cancel**: Abort current plan
- **update**: Update plan content while in plan mode
- **status**: Check current plan state

## When to Use Plan Mode

Use `action: "start"` for implementation tasks unless they're simple:

1. **New Feature Implementation**: Adding meaningful new functionality
2. **Multiple Valid Approaches**: Task can be solved in several ways
3. **Code Modifications**: Changes affecting existing behavior
4. **Architectural Decisions**: Choosing between patterns or technologies
5. **Multi-File Changes**: Task touching more than 2-3 files
6. **Unclear Requirements**: Need exploration before understanding scope

## When NOT to Use

Skip for simple tasks:
- Single-line fixes (typos, obvious bugs)
- Adding a single function with clear requirements
- Tasks with specific, detailed instructions
- Pure research (use Task tool with explore agent)

## Workflow

1. Call with `action: "start"` to enter plan mode
2. Explore codebase using Glob, Grep, Read tools
3. Call with `action: "update"` to record your plan
4. Call with `action: "complete"` to finalize and proceed

## Examples

```json
// Start planning
{"action": "start", "name": "Add user authentication"}

// Update plan content
{"action": "update", "content": "1. Add JWT middleware\n2. Create auth routes"}

// Complete and proceed
{"action": "complete"}

// Check status
{"action": "status"}

// Cancel if needed
{"action": "cancel"}
```

## Integration

- Use Plan for high-level approach and exploration
- Use TodoWrite for granular task tracking during execution
- Plan content persists across session compaction"#;

    async fn handle(&self, input: PlanInput, _context: &ExecutionContext) -> ToolResult {
        match input.action {
            PlanAction::Start => self.start(input.name).await,
            PlanAction::Complete => self.complete().await,
            PlanAction::Cancel => self.cancel().await,
            PlanAction::Update => self.update(input.content).await,
            PlanAction::Status => self.status().await,
        }
    }
}

impl PlanTool {
    async fn start(&self, name: Option<String>) -> ToolResult {
        if self.state.is_in_plan_mode().await {
            return ToolResult::error(
                "Already in plan mode. Complete or cancel the current plan first.",
            );
        }

        let plan = self.state.enter_plan_mode(name).await;
        ToolResult::success(format!(
            "Plan mode started.\n\
            Plan ID: {}\n\
            Status: {:?}\n\n\
            Explore the codebase and design your approach.\n\
            Use action: \"update\" to record your plan.\n\
            Use action: \"complete\" when ready to proceed.",
            plan.id, plan.status
        ))
    }

    async fn complete(&self) -> ToolResult {
        if !self.state.is_in_plan_mode().await {
            return ToolResult::error("No active plan. Use action: \"start\" first.");
        }

        match self.state.exit_plan_mode().await {
            Some(plan) => {
                let content = if plan.content.is_empty() {
                    "No plan content recorded.".to_string()
                } else {
                    plan.content.clone()
                };

                ToolResult::success(format!(
                    "Plan completed.\n\
                    Plan ID: {}\n\
                    Name: {}\n\
                    Status: {:?}\n\n\
                    ## Content\n\n{}\n\n\
                    Proceed with implementation.",
                    plan.id,
                    plan.name.as_deref().unwrap_or("Unnamed"),
                    plan.status,
                    content
                ))
            }
            None => ToolResult::error("No active plan found."),
        }
    }

    async fn cancel(&self) -> ToolResult {
        if !self.state.is_in_plan_mode().await {
            return ToolResult::error("No active plan to cancel.");
        }

        match self.state.cancel_plan().await {
            Some(plan) => ToolResult::success(format!(
                "Plan cancelled.\n\
                Plan ID: {}\n\
                Status: {:?}",
                plan.id, plan.status
            )),
            None => ToolResult::error("No active plan found."),
        }
    }

    async fn update(&self, content: Option<String>) -> ToolResult {
        if !self.state.is_in_plan_mode().await {
            return ToolResult::error("No active plan. Use action: \"start\" first.");
        }

        let content = match content {
            Some(c) if !c.is_empty() => c,
            _ => return ToolResult::error("Content is required for update action."),
        };

        self.state.update_plan_content(content.clone()).await;
        ToolResult::success(format!(
            "Plan content updated.\n\n## Content\n\n{}",
            content
        ))
    }

    async fn status(&self) -> ToolResult {
        match self.state.current_plan().await {
            Some(plan) => {
                let content_preview = if plan.content.is_empty() {
                    "No content recorded.".to_string()
                } else if plan.content.len() > 500 {
                    // Find valid UTF-8 char boundary at or before 500
                    let mut end = 500;
                    while !plan.content.is_char_boundary(end) && end > 0 {
                        end -= 1;
                    }
                    format!("{}...", &plan.content[..end])
                } else {
                    plan.content.clone()
                };

                ToolResult::success(format!(
                    "Plan Status\n\
                    Plan ID: {}\n\
                    Name: {}\n\
                    Status: {:?}\n\
                    Created: {}\n\n\
                    ## Content Preview\n\n{}",
                    plan.id,
                    plan.name.as_deref().unwrap_or("Unnamed"),
                    plan.status,
                    plan.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
                    content_preview
                ))
            }
            None => ToolResult::success("No active plan."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionId;
    use crate::tools::Tool;

    fn test_context() -> ExecutionContext {
        ExecutionContext::default()
    }

    #[tokio::test]
    async fn test_plan_lifecycle() {
        let tool_state = ToolState::new(SessionId::new());
        let tool = PlanTool::new(tool_state);
        let context = test_context();

        // Start
        let result = tool
            .execute(
                serde_json::json!({"action": "start", "name": "Test Plan"}),
                &context,
            )
            .await;
        assert!(!result.is_error());
        assert!(result.text().contains("Plan mode started"));

        // Update
        let result = tool
            .execute(
                serde_json::json!({"action": "update", "content": "Step 1\nStep 2"}),
                &context,
            )
            .await;
        assert!(!result.is_error());
        assert!(result.text().contains("Plan content updated"));

        // Status
        let result = tool
            .execute(serde_json::json!({"action": "status"}), &context)
            .await;
        assert!(!result.is_error());
        assert!(result.text().contains("Step 1"));

        // Complete
        let result = tool
            .execute(serde_json::json!({"action": "complete"}), &context)
            .await;
        assert!(!result.is_error());
        assert!(result.text().contains("Plan completed"));
    }

    #[tokio::test]
    async fn test_plan_cancel() {
        let tool_state = ToolState::new(SessionId::new());
        let tool = PlanTool::new(tool_state);
        let context = test_context();

        // Start
        let result = tool
            .execute(serde_json::json!({"action": "start"}), &context)
            .await;
        assert!(!result.is_error());

        // Cancel
        let result = tool
            .execute(serde_json::json!({"action": "cancel"}), &context)
            .await;
        assert!(!result.is_error());
        assert!(result.text().contains("Plan cancelled"));

        // Status after cancel
        let result = tool
            .execute(serde_json::json!({"action": "status"}), &context)
            .await;
        assert!(result.text().contains("No active plan"));
    }

    #[tokio::test]
    async fn test_double_start_rejected() {
        let tool_state = ToolState::new(SessionId::new());
        let tool = PlanTool::new(tool_state);
        let context = test_context();

        let _ = tool
            .execute(serde_json::json!({"action": "start"}), &context)
            .await;

        let result = tool
            .execute(serde_json::json!({"action": "start"}), &context)
            .await;
        assert!(result.is_error());
        assert!(result.text().contains("Already in plan mode"));
    }

    #[tokio::test]
    async fn test_complete_without_start() {
        let tool_state = ToolState::new(SessionId::new());
        let tool = PlanTool::new(tool_state);
        let context = test_context();

        let result = tool
            .execute(serde_json::json!({"action": "complete"}), &context)
            .await;
        assert!(result.is_error());
        assert!(result.text().contains("No active plan"));
    }

    #[tokio::test]
    async fn test_update_requires_content() {
        let tool_state = ToolState::new(SessionId::new());
        let tool = PlanTool::new(tool_state);
        let context = test_context();

        let _ = tool
            .execute(serde_json::json!({"action": "start"}), &context)
            .await;

        let result = tool
            .execute(serde_json::json!({"action": "update"}), &context)
            .await;
        assert!(result.is_error());
        assert!(result.text().contains("Content is required"));
    }
}
