//! KillShell tool - terminates background shell processes.

use async_trait::async_trait;
use serde::Deserialize;

use super::{Tool, ToolResult};

/// Tool for killing background shell processes
pub struct KillShellTool;

impl KillShellTool {
    /// Create a new KillShell tool
    pub fn new() -> Self {
        Self
    }
}

impl Default for KillShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct KillShellInput {
    shell_id: String,
}

#[async_trait]
impl Tool for KillShellTool {
    fn name(&self) -> &str {
        "KillShell"
    }

    fn description(&self) -> &str {
        "Kills a running background bash shell by its ID. Use this tool when you need to \
         terminate a long-running shell. Shell IDs can be found using the /tasks command."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "shell_id": {
                    "type": "string",
                    "description": "The ID of the background shell to kill"
                }
            },
            "required": ["shell_id"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: KillShellInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // In a full implementation, this would track background processes
        // and kill them by ID. For now, we return a placeholder response.
        // Background process management would be handled by a ProcessManager
        // that maintains a map of shell_id -> process handle.

        // Attempt to parse shell_id as a PID and kill it
        if let Ok(pid) = input.shell_id.parse::<i32>() {
            #[cfg(unix)]
            {
                use std::process::Command;
                let output = Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .output();

                match output {
                    Ok(out) => {
                        if out.status.success() {
                            return ToolResult::success(format!(
                                "Successfully sent TERM signal to process {}",
                                pid
                            ));
                        } else {
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            return ToolResult::error(format!(
                                "Failed to kill process {}: {}",
                                pid, stderr
                            ));
                        }
                    }
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Failed to execute kill command: {}",
                            e
                        ));
                    }
                }
            }

            #[cfg(not(unix))]
            {
                return ToolResult::error(
                    "KillShell is currently only supported on Unix systems".to_string()
                );
            }
        }

        // If not a numeric PID, treat as a named shell ID
        // This would require a ProcessManager to track named sessions
        ToolResult::error(format!(
            "Shell ID '{}' not found. Background shell tracking is not yet implemented.",
            input.shell_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
