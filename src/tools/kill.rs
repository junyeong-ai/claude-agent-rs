//! KillShell tool - terminates background shell processes.

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;

use super::process::ProcessManager;
use super::{ToolResult, TypedTool};

/// Input for the KillShell tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct KillShellInput {
    /// The ID of the background shell to kill.
    pub shell_id: String,
}

/// Tool for killing background shell processes.
pub struct KillShellTool {
    process_manager: Arc<ProcessManager>,
}

impl KillShellTool {
    /// Create a new KillShell tool with its own ProcessManager.
    pub fn new() -> Self {
        Self {
            process_manager: Arc::new(ProcessManager::new()),
        }
    }

    /// Create a new KillShell tool with shared ProcessManager.
    pub fn with_process_manager(manager: Arc<ProcessManager>) -> Self {
        Self {
            process_manager: manager,
        }
    }

    /// Get the process manager.
    pub fn process_manager(&self) -> &Arc<ProcessManager> {
        &self.process_manager
    }
}

impl Default for KillShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for KillShellTool {
    type Input = KillShellInput;

    const NAME: &'static str = "KillShell";
    const DESCRIPTION: &'static str = "Kills a running background bash shell by its ID. \
        Use this to terminate long-running background processes started with run_in_background=true.";

    async fn handle(&self, input: KillShellInput) -> ToolResult {
        match self.process_manager.kill(&input.shell_id).await {
            Ok(()) => ToolResult::success(format!("Process '{}' terminated", input.shell_id)),
            Err(e) => ToolResult::error(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use std::path::PathBuf;

    #[test]
    fn test_kill_shell_input_parsing() {
        let input: KillShellInput = serde_json::from_value(serde_json::json!({
            "shell_id": "shell_123"
        }))
        .unwrap();

        assert_eq!(input.shell_id, "shell_123");
    }

    #[tokio::test]
    async fn test_kill_nonexistent_shell() {
        let tool = KillShellTool::new();
        let result = tool
            .execute(serde_json::json!({
                "shell_id": "nonexistent_shell"
            }))
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_kill_running_process() {
        let mgr = Arc::new(ProcessManager::new());

        // Spawn a long-running process
        let id = mgr.spawn("sleep 10", &PathBuf::from("/tmp")).await.unwrap();

        // Kill it
        let tool = KillShellTool::with_process_manager(mgr.clone());
        let result = tool
            .execute(serde_json::json!({
                "shell_id": id
            }))
            .await;

        match result {
            ToolResult::Success(msg) => assert!(msg.contains("terminated")),
            _ => panic!("Expected success"),
        }

        // Verify it's gone
        assert!(!mgr.is_running(&id).await);
    }
}
