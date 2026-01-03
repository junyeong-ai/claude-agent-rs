//! Bash tool - shell command execution with security hardening.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

use super::SchemaTool;
use super::context::ExecutionContext;
use super::process::ProcessManager;
use crate::types::ToolResult;

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct BashInput {
    /// The command to execute
    pub command: String,
    /// Clear, concise description of what this command does in 5-10 words, in active voice.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional timeout in milliseconds (max 600000)
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Set to true to run this command in the background. Use TaskOutput to read the output later.
    #[serde(default)]
    pub run_in_background: Option<bool>,
    /// Set this to true to dangerously override sandbox mode and run commands without sandboxing.
    #[serde(default, rename = "dangerouslyDisableSandbox")]
    pub dangerously_disable_sandbox: Option<bool>,
}

pub struct BashTool {
    process_manager: Arc<ProcessManager>,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            process_manager: Arc::new(ProcessManager::new()),
        }
    }

    pub fn with_process_manager(manager: Arc<ProcessManager>) -> Self {
        Self {
            process_manager: manager,
        }
    }

    pub fn process_manager(&self) -> &Arc<ProcessManager> {
        &self.process_manager
    }

    fn should_bypass(&self, input: &BashInput, context: &ExecutionContext) -> bool {
        if input.dangerously_disable_sandbox.unwrap_or(false) {
            return context.can_bypass_sandbox();
        }
        false
    }

    async fn execute_foreground(
        &self,
        command: &str,
        timeout_ms: u64,
        context: &ExecutionContext,
        bypass_sandbox: bool,
    ) -> ToolResult {
        let timeout_duration = Duration::from_millis(timeout_ms);
        let env = context.sanitized_env_with_sandbox();
        let limits = context.resource_limits().clone();

        let wrapped_command = if bypass_sandbox {
            command.to_string()
        } else {
            match context.wrap_command(command) {
                Ok(cmd) => cmd,
                Err(e) => return ToolResult::error(format!("Sandbox error: {}", e)),
            }
        };

        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(&wrapped_command);
        cmd.current_dir(context.root());
        cmd.env_clear();
        cmd.envs(env);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        #[cfg(unix)]
        unsafe {
            cmd.pre_exec(move || {
                let _ = limits.apply();
                Ok(())
            });
        }

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

    async fn execute_background(
        &self,
        command: &str,
        context: &ExecutionContext,
        bypass_sandbox: bool,
    ) -> ToolResult {
        let env = context.sanitized_env_with_sandbox();

        let wrapped_command = if bypass_sandbox {
            command.to_string()
        } else {
            match context.wrap_command(command) {
                Ok(cmd) => cmd,
                Err(e) => return ToolResult::error(format!("Sandbox error: {}", e)),
            }
        };

        match self
            .process_manager
            .spawn_with_env(&wrapped_command, context.root(), env)
            .await
        {
            Ok(id) => ToolResult::success(format!(
                "Background process started with ID: {}\nUse TaskOutput tool to monitor output.",
                id
            )),
            Err(e) => ToolResult::error(e),
        }
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchemaTool for BashTool {
    type Input = BashInput;

    const NAME: &'static str = "Bash";
    const DESCRIPTION: &'static str = r#"Executes a given bash command in a persistent shell session with optional timeout, ensuring proper handling and security measures.

IMPORTANT: This tool is for terminal operations like git, npm, docker, etc. DO NOT use it for file operations (reading, writing, editing, searching, finding files) - use the specialized tools for this instead.

Before executing the command, please follow these steps:

1. Directory Verification:
   - If the command will create new directories or files, first use `ls` to verify the parent directory exists and is the correct location
   - For example, before running "mkdir foo/bar", first use `ls foo` to check that "foo" exists and is the intended parent directory

2. Command Execution:
   - Always quote file paths that contain spaces with double quotes (e.g., cd "path with spaces/file.txt")
   - Examples of proper quoting:
     - cd "/Users/name/My Documents" (correct)
     - cd /Users/name/My Documents (incorrect - will fail)
     - python "/path/with spaces/script.py" (correct)
     - python /path/with spaces/script.py (incorrect - will fail)
   - After ensuring proper quoting, execute the command.
   - Capture the output of the command.

Usage notes:
  - The command argument is required.
  - You can specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). If not specified, commands will timeout after 120000ms (2 minutes).
  - It is very helpful if you write a clear, concise description of what this command does in 5-10 words.
  - If the output exceeds 30000 characters, output will be truncated before being returned to you.
  - You can use the `run_in_background` parameter to run the command in the background, which allows you to continue working while the command runs. You can monitor the output using the Bash tool as it becomes available. You do not need to use '&' at the end of the command when using this parameter.

  - Avoid using Bash with the `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands, unless explicitly instructed or when these commands are truly necessary for the task. Instead, always prefer using the dedicated tools for these commands:
    - File search: Use Glob (NOT find or ls)
    - Content search: Use Grep (NOT grep or rg)
    - Read files: Use Read (NOT cat/head/tail)
    - Edit files: Use Edit (NOT sed/awk)
    - Write files: Use Write (NOT echo >/cat <<EOF)
    - Communication: Output text directly (NOT echo/printf)
  - When issuing multiple commands:
    - If the commands are independent and can run in parallel, make multiple Bash tool calls in a single message. For example, if you need to run "git status" and "git diff", send a single message with two Bash tool calls in parallel.
    - If the commands depend on each other and must run sequentially, use a single Bash call with '&&' to chain them together (e.g., `git add . && git commit -m "message" && git push`). For instance, if one operation must complete before another starts (like mkdir before cp, Write before Bash for git operations, or git add before git commit), run these operations sequentially instead.
    - Use ';' only when you need to run commands sequentially but don't care if earlier commands fail
    - DO NOT use newlines to separate commands (newlines are ok in quoted strings)
  - Try to maintain your current working directory throughout the session by using absolute paths and avoiding usage of `cd`. You may use `cd` if the User explicitly requests it.
    <good-example>
    pytest /foo/bar/tests
    </good-example>
    <bad-example>
    cd /foo/bar && pytest tests
    </bad-example>"#;

    async fn handle(&self, input: BashInput, context: &ExecutionContext) -> ToolResult {
        let bypass = self.should_bypass(&input, context);

        if input.run_in_background.unwrap_or(false) {
            self.execute_background(&input.command, context, bypass)
                .await
        } else {
            let timeout_ms = input.timeout.unwrap_or(120_000).min(600_000);
            self.execute_foreground(&input.command, timeout_ms, context, bypass)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::testing::helpers::TestContext;
    use crate::tools::{ExecutionContext, Tool};
    use crate::types::ToolOutput;

    #[tokio::test]
    async fn test_simple_command() {
        let tool = BashTool::new();
        let context = ExecutionContext::permissive();
        let result = tool
            .execute(
                serde_json::json!({"command": "echo 'hello world'"}),
                &context,
            )
            .await;

        assert!(
            matches!(&result.output, ToolOutput::Success(output) if output.contains("hello world")),
            "Expected success with 'hello world', got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_background_command() {
        let tool = BashTool::new();
        let context = ExecutionContext::permissive();
        let result = tool
            .execute(
                serde_json::json!({
                    "command": "echo done",
                    "run_in_background": true
                }),
                &context,
            )
            .await;

        assert!(
            matches!(&result.output, ToolOutput::Success(output) if output.contains("Background process started")),
            "Expected background process started, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_stderr_output() {
        let tool = BashTool::new();
        let context = ExecutionContext::permissive();
        let result = tool
            .execute(
                serde_json::json!({"command": "echo 'stdout' && echo 'stderr' >&2"}),
                &context,
            )
            .await;

        assert!(
            matches!(&result.output, ToolOutput::Success(output) if output.contains("stdout") && output.contains("stderr")),
            "Expected stdout and stderr, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_exit_code_nonzero() {
        let tool = BashTool::new();
        let context = ExecutionContext::permissive();
        let result = tool
            .execute(serde_json::json!({"command": "exit 42"}), &context)
            .await;

        assert!(
            matches!(&result.output, ToolOutput::Success(output) if output.contains("Exit code: 42")),
            "Expected exit code 42, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_short_timeout() {
        let tool = BashTool::new();
        let context = ExecutionContext::permissive();
        let result = tool
            .execute(
                serde_json::json!({
                    "command": "sleep 10",
                    "timeout": 100
                }),
                &context,
            )
            .await;

        assert!(result.is_error(), "Expected timeout error");
        assert!(
            matches!(&result.output, ToolOutput::Error(e) if e.to_string().contains("timed out")),
            "Expected timeout message, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_working_directory() {
        let test_context = TestContext::new();
        test_context.write_file("testfile.txt", "content");

        let tool = BashTool::new();
        let result = tool
            .execute(
                serde_json::json!({"command": "ls testfile.txt"}),
                &test_context.context,
            )
            .await;

        assert!(
            matches!(&result.output, ToolOutput::Success(output) if output.contains("testfile.txt")),
            "Expected testfile.txt in output, got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_shared_process_manager() {
        let manager = Arc::new(ProcessManager::new());
        let tool1 = BashTool::with_process_manager(manager.clone());
        let tool2 = BashTool::with_process_manager(manager.clone());

        assert!(Arc::ptr_eq(
            tool1.process_manager(),
            tool2.process_manager()
        ));
    }
}
