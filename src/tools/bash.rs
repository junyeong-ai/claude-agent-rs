//! Bash tool - shell command execution.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

use super::process::ProcessManager;
use super::{ToolResult, TypedTool};

/// Input for the Bash tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BashInput {
    /// The command to execute.
    pub command: String,
    /// Timeout in milliseconds (max 600000). Ignored for background commands.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Run the command in the background. Returns a process ID for later management.
    #[serde(default)]
    pub run_in_background: Option<bool>,
}

/// Tool for executing shell commands.
pub struct BashTool {
    working_dir: PathBuf,
    process_manager: Arc<ProcessManager>,
}

impl BashTool {
    /// Create a new Bash tool.
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            process_manager: Arc::new(ProcessManager::new()),
        }
    }

    /// Create a new Bash tool with shared ProcessManager.
    pub fn with_process_manager(working_dir: PathBuf, manager: Arc<ProcessManager>) -> Self {
        Self {
            working_dir,
            process_manager: manager,
        }
    }

    /// Get the process manager.
    pub fn process_manager(&self) -> &Arc<ProcessManager> {
        &self.process_manager
    }

    fn is_dangerous(command: &str) -> bool {
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf ~",
            "dd if=/dev/zero",
            "mkfs",
            "> /dev/sda",
            "chmod -R 777 /",
            ":(){:|:&};:",
        ];
        dangerous_patterns.iter().any(|p| command.contains(p))
    }

    async fn execute_foreground(&self, command: &str, timeout_ms: u64) -> ToolResult {
        let timeout_duration = Duration::from_millis(timeout_ms);

        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(command);
        cmd.current_dir(&self.working_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let result = timeout(timeout_duration, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                let mut combined = String::new();

                if !stdout.is_empty() {
                    combined.push_str(&stdout);
                }

                if !stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push_str("\n--- stderr ---\n");
                    }
                    combined.push_str(&stderr);
                }

                const MAX_OUTPUT: usize = 30_000;
                if combined.len() > MAX_OUTPUT {
                    combined.truncate(MAX_OUTPUT);
                    combined.push_str("\n... (output truncated)");
                }

                if combined.is_empty() {
                    combined = "(no output)".to_string();
                }

                if !output.status.success() {
                    let code = output.status.code().unwrap_or(-1);
                    combined = format!("Exit code: {}\n{}", code, combined);
                }

                ToolResult::success(combined)
            }
            Ok(Err(e)) => ToolResult::error(format!("Failed to execute command: {}", e)),
            Err(_) => ToolResult::error(format!(
                "Command timed out after {} seconds",
                timeout_ms / 1000
            )),
        }
    }

    async fn execute_background(&self, command: &str) -> ToolResult {
        match self.process_manager.spawn(command, &self.working_dir).await {
            Ok(id) => ToolResult::success(format!(
                "Background process started with ID: {}\nUse TaskOutput tool to monitor output.",
                id
            )),
            Err(e) => ToolResult::error(e),
        }
    }
}

#[async_trait]
impl TypedTool for BashTool {
    type Input = BashInput;

    const NAME: &'static str = "Bash";
    const DESCRIPTION: &'static str = "Executes a bash command in a persistent shell session. \
        Default timeout is 120 seconds, maximum is 600 seconds. \
        Use run_in_background=true for long-running commands. \
        Output is truncated at 30,000 characters.";

    async fn handle(&self, input: BashInput) -> ToolResult {
        if Self::is_dangerous(&input.command) {
            return ToolResult::error(
                "This command appears dangerous and has been blocked for safety.",
            );
        }

        if input.run_in_background.unwrap_or(false) {
            self.execute_background(&input.command).await
        } else {
            let timeout_ms = input.timeout.unwrap_or(120_000).min(600_000);
            self.execute_foreground(&input.command, timeout_ms).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn test_dangerous_commands() {
        assert!(BashTool::is_dangerous("rm -rf /"));
        assert!(BashTool::is_dangerous("sudo rm -rf /home"));
        assert!(!BashTool::is_dangerous("ls -la"));
        assert!(!BashTool::is_dangerous("echo hello"));
    }

    #[tokio::test]
    async fn test_simple_command() {
        let tool = BashTool::new(PathBuf::from("/tmp"));
        let result = tool
            .execute(serde_json::json!({
                "command": "echo 'hello world'"
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("hello world"));
            }
            _ => panic!("Expected success"),
        }
    }

    #[tokio::test]
    async fn test_background_command() {
        let tool = BashTool::new(PathBuf::from("/tmp"));
        let result = tool
            .execute(serde_json::json!({
                "command": "sleep 0.1 && echo done",
                "run_in_background": true
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("Background process started"));
                assert!(output.contains("ID:"));
            }
            _ => panic!("Expected success"),
        }
    }

    #[tokio::test]
    async fn test_shared_process_manager() {
        let mgr = Arc::new(ProcessManager::new());
        let tool = BashTool::with_process_manager(PathBuf::from("/tmp"), mgr.clone());

        tool.execute(serde_json::json!({
            "command": "sleep 0.5",
            "run_in_background": true
        }))
        .await;

        let list = mgr.list().await;
        assert_eq!(list.len(), 1);
    }
}
