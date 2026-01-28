//! Hook traits and types.

use crate::types::ToolOutput;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
    Stop,
    SubagentStart,
    SubagentStop,
    PreCompact,
    SessionStart,
    SessionEnd,
}

impl HookEvent {
    /// Returns true if this hook event can block execution.
    ///
    /// Blockable events use fail-closed semantics: if the hook fails or times out,
    /// the operation is blocked. This ensures security policies are enforced.
    ///
    /// Blockable events:
    /// - `PreToolUse`: Can block tool execution
    /// - `UserPromptSubmit`: Can block prompt processing
    /// - `SessionStart`: Can block session initialization
    /// - `PreCompact`: Can block context compaction
    /// - `SubagentStart`: Can block subagent spawning
    pub fn can_block(&self) -> bool {
        matches!(
            self,
            Self::PreToolUse
                | Self::UserPromptSubmit
                | Self::SessionStart
                | Self::PreCompact
                | Self::SubagentStart
        )
    }

    /// Parse a PascalCase event name (as used in hooks.json configs).
    pub fn from_pascal_case(s: &str) -> Option<Self> {
        match s {
            "PreToolUse" => Some(Self::PreToolUse),
            "PostToolUse" => Some(Self::PostToolUse),
            "PostToolUseFailure" => Some(Self::PostToolUseFailure),
            "UserPromptSubmit" => Some(Self::UserPromptSubmit),
            "Stop" => Some(Self::Stop),
            "SubagentStart" => Some(Self::SubagentStart),
            "SubagentStop" => Some(Self::SubagentStop),
            "PreCompact" => Some(Self::PreCompact),
            "SessionStart" => Some(Self::SessionStart),
            "SessionEnd" => Some(Self::SessionEnd),
            _ => None,
        }
    }

    pub fn all() -> &'static [HookEvent] {
        &[
            Self::PreToolUse,
            Self::PostToolUse,
            Self::PostToolUseFailure,
            Self::UserPromptSubmit,
            Self::Stop,
            Self::SubagentStart,
            Self::SubagentStop,
            Self::PreCompact,
            Self::SessionStart,
            Self::SessionEnd,
        ]
    }
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::PreToolUse => "pre_tool_use",
            Self::PostToolUse => "post_tool_use",
            Self::PostToolUseFailure => "post_tool_use_failure",
            Self::UserPromptSubmit => "user_prompt_submit",
            Self::Stop => "stop",
            Self::SubagentStart => "subagent_start",
            Self::SubagentStop => "subagent_stop",
            Self::PreCompact => "pre_compact",
            Self::SessionStart => "session_start",
            Self::SessionEnd => "session_end",
        };
        write!(f, "{}", s)
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum HookEventData {
    PreToolUse {
        tool_name: String,
        tool_input: Value,
    },
    PostToolUse {
        tool_name: String,
        tool_result: ToolOutput,
    },
    PostToolUseFailure {
        tool_name: String,
        error: String,
    },
    UserPromptSubmit {
        prompt: String,
    },
    Stop,
    SubagentStart {
        subagent_id: String,
        subagent_type: String,
        description: String,
    },
    SubagentStop {
        subagent_id: String,
        success: bool,
        error: Option<String>,
    },
    PreCompact,
    SessionStart,
    SessionEnd,
}

impl HookEventData {
    pub fn event_type(&self) -> HookEvent {
        match self {
            Self::PreToolUse { .. } => HookEvent::PreToolUse,
            Self::PostToolUse { .. } => HookEvent::PostToolUse,
            Self::PostToolUseFailure { .. } => HookEvent::PostToolUseFailure,
            Self::UserPromptSubmit { .. } => HookEvent::UserPromptSubmit,
            Self::Stop => HookEvent::Stop,
            Self::SubagentStart { .. } => HookEvent::SubagentStart,
            Self::SubagentStop { .. } => HookEvent::SubagentStop,
            Self::PreCompact => HookEvent::PreCompact,
            Self::SessionStart => HookEvent::SessionStart,
            Self::SessionEnd => HookEvent::SessionEnd,
        }
    }

    pub fn tool_name(&self) -> Option<&str> {
        match self {
            Self::PreToolUse { tool_name, .. }
            | Self::PostToolUse { tool_name, .. }
            | Self::PostToolUseFailure { tool_name, .. } => Some(tool_name),
            _ => None,
        }
    }

    pub fn tool_input(&self) -> Option<&Value> {
        match self {
            Self::PreToolUse { tool_input, .. } => Some(tool_input),
            _ => None,
        }
    }

    pub fn subagent_id(&self) -> Option<&str> {
        match self {
            Self::SubagentStart { subagent_id, .. } | Self::SubagentStop { subagent_id, .. } => {
                Some(subagent_id)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HookInput {
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub data: HookEventData,
    pub metadata: Option<Value>,
}

impl HookInput {
    pub fn new(session_id: impl Into<String>, data: HookEventData) -> Self {
        Self {
            session_id: session_id.into(),
            timestamp: Utc::now(),
            data,
            metadata: None,
        }
    }

    pub fn event_type(&self) -> HookEvent {
        self.data.event_type()
    }

    pub fn tool_name(&self) -> Option<&str> {
        self.data.tool_name()
    }

    pub fn subagent_id(&self) -> Option<&str> {
        self.data.subagent_id()
    }

    pub fn pre_tool_use(
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        tool_input: Value,
    ) -> Self {
        Self::new(
            session_id,
            HookEventData::PreToolUse {
                tool_name: tool_name.into(),
                tool_input,
            },
        )
    }

    pub fn post_tool_use(
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        tool_result: ToolOutput,
    ) -> Self {
        Self::new(
            session_id,
            HookEventData::PostToolUse {
                tool_name: tool_name.into(),
                tool_result,
            },
        )
    }

    pub fn post_tool_use_failure(
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self::new(
            session_id,
            HookEventData::PostToolUseFailure {
                tool_name: tool_name.into(),
                error: error.into(),
            },
        )
    }

    pub fn user_prompt_submit(session_id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self::new(
            session_id,
            HookEventData::UserPromptSubmit {
                prompt: prompt.into(),
            },
        )
    }

    pub fn session_start(session_id: impl Into<String>) -> Self {
        Self::new(session_id, HookEventData::SessionStart)
    }

    pub fn session_end(session_id: impl Into<String>) -> Self {
        Self::new(session_id, HookEventData::SessionEnd)
    }

    pub fn stop(session_id: impl Into<String>) -> Self {
        Self::new(session_id, HookEventData::Stop)
    }

    pub fn pre_compact(session_id: impl Into<String>) -> Self {
        Self::new(session_id, HookEventData::PreCompact)
    }

    pub fn subagent_start(
        session_id: impl Into<String>,
        subagent_id: impl Into<String>,
        subagent_type: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self::new(
            session_id,
            HookEventData::SubagentStart {
                subagent_id: subagent_id.into(),
                subagent_type: subagent_type.into(),
                description: description.into(),
            },
        )
    }

    pub fn subagent_stop(
        session_id: impl Into<String>,
        subagent_id: impl Into<String>,
        success: bool,
        error: Option<String>,
    ) -> Self {
        Self::new(
            session_id,
            HookEventData::SubagentStop {
                subagent_id: subagent_id.into(),
                success,
                error,
            },
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct HookOutput {
    pub continue_execution: bool,
    pub stop_reason: Option<String>,
    pub suppress_logging: bool,
    pub system_message: Option<String>,
    pub updated_input: Option<Value>,
    pub additional_context: Option<String>,
}

impl HookOutput {
    pub fn allow() -> Self {
        Self {
            continue_execution: true,
            ..Default::default()
        }
    }

    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            continue_execution: false,
            stop_reason: Some(reason.into()),
            ..Default::default()
        }
    }

    pub fn with_system_message(mut self, message: impl Into<String>) -> Self {
        self.system_message = Some(message.into());
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.additional_context = Some(context.into());
        self
    }

    pub fn with_updated_input(mut self, input: Value) -> Self {
        self.updated_input = Some(input);
        self
    }

    pub fn suppress_logging(mut self) -> Self {
        self.suppress_logging = true;
        self
    }
}

#[derive(Clone, Debug)]
pub struct HookContext {
    pub session_id: String,
    pub cancellation_token: CancellationToken,
    pub cwd: Option<std::path::PathBuf>,
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
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Default::default()
        }
    }

    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }

    pub fn with_cwd(mut self, cwd: impl Into<std::path::PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_env(mut self, env: std::collections::HashMap<String, String>) -> Self {
        self.env = env;
        self
    }
}

/// Hook metadata for configuration.
#[derive(Clone, Debug)]
pub struct HookMetadata {
    pub name: String,
    pub events: Vec<HookEvent>,
    pub priority: i32,
    pub timeout_secs: u64,
    pub tool_matcher: Option<Regex>,
}

impl HookMetadata {
    pub fn new(name: impl Into<String>, events: Vec<HookEvent>) -> Self {
        Self {
            name: name.into(),
            events,
            priority: 0,
            timeout_secs: 60,
            tool_matcher: None,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn with_tool_matcher(mut self, pattern: &str) -> Self {
        if let Ok(regex) = Regex::new(pattern) {
            self.tool_matcher = Some(regex);
        }
        self
    }
}

#[async_trait]
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    fn events(&self) -> &[HookEvent];

    #[inline]
    fn tool_matcher(&self) -> Option<&Regex> {
        None
    }

    #[inline]
    fn timeout_secs(&self) -> u64 {
        60
    }

    #[inline]
    fn priority(&self) -> i32 {
        0
    }

    async fn execute(
        &self,
        input: HookInput,
        hook_context: &HookContext,
    ) -> Result<HookOutput, crate::Error>;

    /// Get full metadata as a struct.
    fn metadata(&self) -> HookMetadata {
        HookMetadata {
            name: self.name().to_string(),
            events: self.events().to_vec(),
            priority: self.priority(),
            timeout_secs: self.timeout_secs(),
            tool_matcher: self.tool_matcher().cloned(),
        }
    }
}

pub struct FnHook<F> {
    name: String,
    events: Vec<HookEvent>,
    handler: F,
    priority: i32,
    timeout_secs: u64,
    tool_matcher: Option<Regex>,
}

impl<F> FnHook<F> {
    pub fn builder(name: impl Into<String>, events: Vec<HookEvent>) -> FnHookBuilder {
        FnHookBuilder {
            name: name.into(),
            events,
            priority: 0,
            timeout_secs: 60,
            tool_matcher: None,
        }
    }
}

pub struct FnHookBuilder {
    name: String,
    events: Vec<HookEvent>,
    priority: i32,
    timeout_secs: u64,
    tool_matcher: Option<Regex>,
}

impl FnHookBuilder {
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn tool_matcher(mut self, pattern: &str) -> Self {
        if let Ok(regex) = Regex::new(pattern) {
            self.tool_matcher = Some(regex);
        }
        self
    }

    pub fn handler<F, Fut>(self, handler: F) -> FnHook<F>
    where
        F: Fn(HookInput, HookContext) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<HookOutput, crate::Error>> + Send,
    {
        FnHook {
            name: self.name,
            events: self.events,
            handler,
            priority: self.priority,
            timeout_secs: self.timeout_secs,
            tool_matcher: self.tool_matcher,
        }
    }
}

#[async_trait]
impl<F, Fut> Hook for FnHook<F>
where
    F: Fn(HookInput, HookContext) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<HookOutput, crate::Error>> + Send,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn events(&self) -> &[HookEvent] {
        &self.events
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn timeout_secs(&self) -> u64 {
        self.timeout_secs
    }

    fn tool_matcher(&self) -> Option<&Regex> {
        self.tool_matcher.as_ref()
    }

    async fn execute(
        &self,
        input: HookInput,
        hook_context: &HookContext,
    ) -> Result<HookOutput, crate::Error> {
        (self.handler)(input, hook_context.clone()).await
    }
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
        // Blockable events (fail-closed semantics)
        assert!(HookEvent::PreToolUse.can_block());
        assert!(HookEvent::UserPromptSubmit.can_block());
        assert!(HookEvent::SessionStart.can_block());
        assert!(HookEvent::PreCompact.can_block());
        assert!(HookEvent::SubagentStart.can_block());

        // Non-blockable events (fail-open semantics)
        assert!(!HookEvent::PostToolUse.can_block());
        assert!(!HookEvent::PostToolUseFailure.can_block());
        assert!(!HookEvent::SessionEnd.can_block());
        assert!(!HookEvent::SubagentStop.can_block());
        assert!(!HookEvent::Stop.can_block());
    }

    #[test]
    fn test_hook_input_builders() {
        let input =
            HookInput::pre_tool_use("session-1", "Read", serde_json::json!({"path": "/tmp"}));
        assert_eq!(input.event_type(), HookEvent::PreToolUse);
        assert_eq!(input.tool_name(), Some("Read"));
        assert_eq!(input.session_id, "session-1");

        let input = HookInput::session_start("session-2");
        assert_eq!(input.event_type(), HookEvent::SessionStart);
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
            .suppress_logging();
        assert!(output.continue_execution);
        assert!(output.suppress_logging);
        assert_eq!(output.system_message, Some("Added context".to_string()));
        assert_eq!(output.additional_context, Some("More info".to_string()));
    }

    #[test]
    fn test_hook_event_data_accessors() {
        let data = HookEventData::PreToolUse {
            tool_name: "Bash".to_string(),
            tool_input: serde_json::json!({"command": "ls"}),
        };
        assert_eq!(data.event_type(), HookEvent::PreToolUse);
        assert_eq!(data.tool_name(), Some("Bash"));
        assert!(data.tool_input().is_some());

        let data = HookEventData::SessionStart;
        assert_eq!(data.event_type(), HookEvent::SessionStart);
        assert_eq!(data.tool_name(), None);
        assert!(data.tool_input().is_none());
    }
}
