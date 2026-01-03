//! Tool execution environment.

use std::sync::Arc;

use super::ProcessManager;
use super::context::ExecutionContext;
use crate::session::session_state::ToolState;

/// Execution environment for tools including security context and shared state.
#[derive(Clone)]
pub struct ToolExecutionEnv {
    pub context: ExecutionContext,
    pub tool_state: Option<ToolState>,
    pub process_manager: Option<Arc<ProcessManager>>,
}

impl ToolExecutionEnv {
    pub fn new(context: ExecutionContext) -> Self {
        Self {
            context,
            tool_state: None,
            process_manager: None,
        }
    }

    pub fn with_tool_state(mut self, state: ToolState) -> Self {
        self.tool_state = Some(state);
        self
    }

    pub fn with_process_manager(mut self, pm: Arc<ProcessManager>) -> Self {
        self.process_manager = Some(pm);
        self
    }
}

impl Default for ToolExecutionEnv {
    fn default() -> Self {
        Self::new(ExecutionContext::default())
    }
}
