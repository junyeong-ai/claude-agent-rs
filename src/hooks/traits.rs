//! Hook traits and types.

use crate::permissions::PermissionDecision;
use crate::tools::ToolResult;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

/// Hook event types that trigger hook execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Before a tool is executed (can block or modify input)
    PreToolUse,

    /// After successful tool execution
    PostToolUse,

    /// After failed tool execution
    PostToolUseFailure,

    /// When user submits a prompt
    UserPromptSubmit,

    /// When agent stops execution
    Stop,

    /// When a subagent is spawned
    SubagentStart,

    /// When a subagent completes
    SubagentStop,

    /// Before conversation compaction
    PreCompact,

    /// When a session begins
    SessionStart,

    /// When a session ends
    SessionEnd,

    /// For custom notifications
    Notification,

    /// When permission is requested (for custom permission handling)
    PermissionRequest,
}

impl HookEvent {
    /// Check if this event can block execution
    pub fn can_block(&self) -> bool {
        matches!(
            self,
            HookEvent::PreToolUse | HookEvent::UserPromptSubmit | HookEvent::PermissionRequest
        )
    }

    /// Check if this event can modify input
    pub fn can_modify_input(&self) -> bool {
        matches!(self, HookEvent::PreToolUse | HookEvent::UserPromptSubmit)
    }

    /// Get all possible hook events
    pub fn all() -> &'static [HookEvent] {
        &[
            HookEvent::PreToolUse,
            HookEvent::PostToolUse,
            HookEvent::PostToolUseFailure,
            HookEvent::UserPromptSubmit,
            HookEvent::Stop,
            HookEvent::SubagentStart,
            HookEvent::SubagentStop,
            HookEvent::PreCompact,
            HookEvent::SessionStart,
            HookEvent::SessionEnd,
            HookEvent::Notification,
            HookEvent::PermissionRequest,
        ]
    }
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookEvent::PreToolUse => write!(f, "pre_tool_use"),
            HookEvent::PostToolUse => write!(f, "post_tool_use"),
            HookEvent::PostToolUseFailure => write!(f, "post_tool_use_failure"),
            HookEvent::UserPromptSubmit => write!(f, "user_prompt_submit"),
            HookEvent::Stop => write!(f, "stop"),
            HookEvent::SubagentStart => write!(f, "subagent_start"),
            HookEvent::SubagentStop => write!(f, "subagent_stop"),
            HookEvent::PreCompact => write!(f, "pre_compact"),
            HookEvent::SessionStart => write!(f, "session_start"),
            HookEvent::SessionEnd => write!(f, "session_end"),
            HookEvent::Notification => write!(f, "notification"),
            HookEvent::PermissionRequest => write!(f, "permission_request"),
        }
    }
}

/// Input data for hook execution.
#[derive(Clone, Debug, Default)]
pub struct HookInput {
    /// The event that triggered this hook
    pub event: Option<HookEvent>,

    /// Tool name (for tool-related events)
    pub tool_name: Option<String>,

    /// Tool input (for PreToolUse)
    pub tool_input: Option<Value>,

    /// Tool result (for PostToolUse/PostToolUseFailure)
    pub tool_result: Option<ToolResult>,

    /// Session ID
    pub session_id: String,

    /// User prompt (for UserPromptSubmit)
    pub user_prompt: Option<String>,

    /// Subagent ID (for subagent events)
    pub subagent_id: Option<String>,

    /// Subagent definition (for SubagentStart)
    pub subagent_definition: Option<Value>,

    /// Notification message (for Notification events)
    pub notification: Option<String>,

    /// Error message (for failure events)
    pub error: Option<String>,

    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,

    /// Additional metadata
    pub metadata: Option<Value>,
}

impl HookInput {
    /// Create a new hook input for a pre-tool-use event
    pub fn pre_tool_use(
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        input: Value,
    ) -> Self {
        Self {
            event: Some(HookEvent::PreToolUse),
            tool_name: Some(tool_name.into()),
            tool_input: Some(input),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    /// Create a new hook input for a post-tool-use event
    pub fn post_tool_use(
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        result: ToolResult,
    ) -> Self {
        Self {
            event: Some(HookEvent::PostToolUse),
            tool_name: Some(tool_name.into()),
            tool_result: Some(result),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    /// Create a new hook input for a post-tool-use-failure event
    pub fn post_tool_use_failure(
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            event: Some(HookEvent::PostToolUseFailure),
            tool_name: Some(tool_name.into()),
            error: Some(error.into()),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    /// Create a new hook input for a user prompt submit event
    pub fn user_prompt_submit(session_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            event: Some(HookEvent::UserPromptSubmit),
            user_prompt: Some(prompt.into()),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    /// Create a new hook input for a session start event
    pub fn session_start(session_id: impl Into<String>) -> Self {
        Self {
            event: Some(HookEvent::SessionStart),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    /// Create a new hook input for a session end event
    pub fn session_end(session_id: impl Into<String>) -> Self {
        Self {
            event: Some(HookEvent::SessionEnd),
            session_id: session_id.into(),
            timestamp: Utc::now(),
            ..Default::default()
        }
    }
}

/// Output from hook execution.
#[derive(Clone, Debug, Default)]
pub struct HookOutput {
    /// Whether to continue execution (false = block)
    pub continue_execution: bool,

    /// Reason for stopping execution (if continue_execution is false)
    pub stop_reason: Option<String>,

    /// Whether to suppress output/logging
    pub suppress_output: bool,

    /// System message to inject into context
    pub system_message: Option<String>,

    /// Permission decision override
    pub permission_decision: Option<PermissionDecision>,

    /// Modified input (for PreToolUse, replaces original input)
    pub updated_input: Option<Value>,

    /// Additional context to add
    pub additional_context: Option<String>,

    /// User message to display
    pub user_message: Option<String>,
}

impl HookOutput {
    /// Create an output that allows execution to continue
    pub fn allow() -> Self {
        Self {
            continue_execution: true,
            ..Default::default()
        }
    }

    /// Create an output that blocks execution
    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            continue_execution: false,
            stop_reason: Some(reason.into()),
            ..Default::default()
        }
    }

    /// Create an output that allows with a permission decision
    pub fn with_permission(decision: PermissionDecision) -> Self {
        Self {
            continue_execution: true,
            permission_decision: Some(decision),
            ..Default::default()
        }
    }

    /// Set the system message to inject
    pub fn with_system_message(mut self, message: impl Into<String>) -> Self {
        self.system_message = Some(message.into());
        self
    }

    /// Set the additional context
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.additional_context = Some(context.into());
        self
    }

    /// Set the updated input (for PreToolUse)
    pub fn with_updated_input(mut self, input: Value) -> Self {
        self.updated_input = Some(input);
        self
    }

    /// Set a user message to display
    pub fn with_user_message(mut self, message: impl Into<String>) -> Self {
        self.user_message = Some(message.into());
        self
    }

    /// Suppress output/logging
    pub fn suppress(mut self) -> Self {
        self.suppress_output = true;
        self
    }
}

/// Context provided to hook execution.
#[derive(Clone, Debug)]
pub struct HookContext {
    /// Session ID
    pub session_id: String,

    /// Cancellation token for async operations
    pub cancellation_token: CancellationToken,

    /// Current working directory
    pub cwd: Option<std::path::PathBuf>,

    /// Environment variables
    pub env: std::collections::HashMap<String, String>,
}

impl Default for HookContext {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            cancellation_token: CancellationToken::new(),
            cwd: None,
            env: std::collections::HashMap::new(),
        }
    }
}

impl HookContext {
    /// Create a new hook context
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Default::default()
        }
    }

    /// Set the cancellation token
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }

    /// Set the working directory
    pub fn with_cwd(mut self, cwd: impl Into<std::path::PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}

/// Trait for implementing hooks.
///
/// Hooks are executed at specific points in the agent lifecycle and can:
/// - Block or allow tool executions
/// - Modify tool inputs
/// - Inject context or system messages
/// - Log and audit operations
///
/// # Example
///
/// ```rust,no_run
/// use claude_agent::hooks::{Hook, HookEvent, HookInput, HookOutput, HookContext};
/// use async_trait::async_trait;
///
/// struct SecurityHook {
///     blocked_commands: Vec<String>,
/// }
///
/// #[async_trait]
/// impl Hook for SecurityHook {
///     fn name(&self) -> &str {
///         "security-hook"
///     }
///
///     fn events(&self) -> &[HookEvent] {
///         &[HookEvent::PreToolUse]
///     }
///
///     async fn execute(&self, input: HookInput, _ctx: &HookContext)
///         -> Result<HookOutput, claude_agent::Error>
///     {
///         if let Some(tool_name) = &input.tool_name {
///             if tool_name == "Bash" {
///                 if let Some(tool_input) = &input.tool_input {
///                     if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
///                         for blocked in &self.blocked_commands {
///                             if cmd.contains(blocked) {
///                                 return Ok(HookOutput::block(
///                                     format!("Command contains blocked pattern: {}", blocked)
///                                 ));
///                             }
///                         }
///                     }
///                 }
///             }
///         }
///         Ok(HookOutput::allow())
///     }
/// }
/// ```
#[async_trait]
pub trait Hook: Send + Sync {
    /// Get the unique name of this hook
    fn name(&self) -> &str;

    /// Get the events this hook handles
    fn events(&self) -> &[HookEvent];

    /// Get the tool name matcher (regex pattern).
    /// If None, the hook applies to all tools.
    fn tool_matcher(&self) -> Option<&Regex> {
        None
    }

    /// Get the timeout for this hook in seconds
    fn timeout_secs(&self) -> u64 {
        60
    }

    /// Get the priority of this hook (higher = runs first)
    fn priority(&self) -> i32 {
        0
    }

    /// Execute the hook
    async fn execute(
        &self,
        input: HookInput,
        ctx: &HookContext,
    ) -> Result<HookOutput, crate::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_display() {
        assert_eq!(HookEvent::PreToolUse.to_string(), "pre_tool_use");
        assert_eq!(HookEvent::PostToolUse.to_string(), "post_tool_use");
        assert_eq!(HookEvent::SessionStart.to_string(), "session_start");
    }

    #[test]
    fn test_hook_event_can_block() {
        assert!(HookEvent::PreToolUse.can_block());
        assert!(HookEvent::UserPromptSubmit.can_block());
        assert!(HookEvent::PermissionRequest.can_block());
        assert!(!HookEvent::PostToolUse.can_block());
        assert!(!HookEvent::SessionEnd.can_block());
    }

    #[test]
    fn test_hook_input_builders() {
        let input =
            HookInput::pre_tool_use("session-1", "Read", serde_json::json!({"path": "/tmp"}));
        assert_eq!(input.event, Some(HookEvent::PreToolUse));
        assert_eq!(input.tool_name, Some("Read".to_string()));
        assert_eq!(input.session_id, "session-1");

        let input = HookInput::session_start("session-2");
        assert_eq!(input.event, Some(HookEvent::SessionStart));
        assert_eq!(input.session_id, "session-2");
    }

    #[test]
    fn test_hook_output_builders() {
        let output = HookOutput::allow();
        assert!(output.continue_execution);
        assert!(output.stop_reason.is_none());

        let output = HookOutput::block("Dangerous operation");
        assert!(!output.continue_execution);
        assert_eq!(output.stop_reason, Some("Dangerous operation".to_string()));

        let output = HookOutput::allow()
            .with_system_message("Added context")
            .with_context("More info")
            .suppress();
        assert!(output.continue_execution);
        assert!(output.suppress_output);
        assert_eq!(output.system_message, Some("Added context".to_string()));
        assert_eq!(output.additional_context, Some("More info".to_string()));
    }
}
