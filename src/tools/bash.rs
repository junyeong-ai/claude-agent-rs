//! Bash tool - shell command execution.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

use super::{Tool, ToolResult};

/// Tool for executing shell commands
pub struct BashTool {
    working_dir: PathBuf,
}

impl BashTool {
    /// Create a new Bash tool
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Check if a command is potentially dangerous
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
}

#[derive(Debug, Deserialize)]
struct BashInput {
    command: String,
    #[serde(default)]
    timeout: Option<u64>,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Executes a bash command in a persistent shell session with optional timeout. \
         Default timeout is 120 seconds, maximum is 600 seconds. \
         Output is truncated at 30,000 characters."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Timeout in milliseconds (max 600000)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input: BashInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return ToolResult::error(format!("Invalid input: {}", e)),
        };

        // Safety check
        if Self::is_dangerous(&input.command) {
            return ToolResult::error(
                "This command appears dangerous and has been blocked for safety. \
                 If you believe this is safe, please ask the user for explicit approval.",
            );
        }

        // Calculate timeout
        let timeout_ms = input.timeout.unwrap_or(120_000).min(600_000);
        let timeout_duration = Duration::from_millis(timeout_ms);

        // Build command
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&input.command);
        cmd.current_dir(&self.working_dir);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Execute with timeout
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

                // Truncate if too long
                const MAX_OUTPUT: usize = 30_000;
                if combined.len() > MAX_OUTPUT {
                    combined.truncate(MAX_OUTPUT);
                    combined.push_str("\n... (output truncated)");
                }

                if combined.is_empty() {
                    combined = "(no output)".to_string();
                }

                // Include exit code if non-zero
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
