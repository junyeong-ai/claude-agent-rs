//! Session state management.

mod config;
mod enums;
mod ids;
mod message;
mod policy;

pub use config::SessionConfig;
pub use enums::{SessionMode, SessionState, SessionType};
pub use ids::{MessageId, SessionId};
pub use message::{MessageMetadata, SessionMessage, ThinkingMetadata, ToolResultMeta};
pub use policy::{PermissionMode, PermissionPolicy, ToolLimits};

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::session::types::{CompactRecord, Plan, TodoItem, TodoStatus};
use crate::types::{CacheControl, CacheTtl, ContentBlock, Message, Role, TokenUsage, Usage};

const MAX_COMPACT_HISTORY_SIZE: usize = 50;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub parent_id: Option<SessionId>,
    pub session_type: SessionType,
    pub tenant_id: Option<String>,
    pub mode: SessionMode,
    pub state: SessionState,
    pub config: SessionConfig,
    pub permission_policy: PermissionPolicy,
    pub messages: Vec<SessionMessage>,
    pub current_leaf_id: Option<MessageId>,
    pub summary: Option<String>,
    pub total_usage: TokenUsage,
    #[serde(default)]
    pub current_input_tokens: u64,
    pub total_cost_usd: f64,
    pub static_context_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    #[serde(default)]
    pub todos: Vec<TodoItem>,
    #[serde(default)]
    pub current_plan: Option<Plan>,
    #[serde(default)]
    pub compact_history: Vec<CompactRecord>,
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        Self::with_id(SessionId::new(), config)
    }

    pub fn with_id(id: SessionId, config: SessionConfig) -> Self {
        Self::init(id, None, SessionType::Main, config)
    }

    pub fn new_subagent(
        parent_id: SessionId,
        agent_type: impl Into<String>,
        description: impl Into<String>,
        config: SessionConfig,
    ) -> Self {
        let session_type = SessionType::Subagent {
            agent_type: agent_type.into(),
            description: description.into(),
        };
        Self::init(SessionId::new(), Some(parent_id), session_type, config)
    }

    fn init(
        id: SessionId,
        parent_id: Option<SessionId>,
        session_type: SessionType,
        config: SessionConfig,
    ) -> Self {
        let now = Utc::now();
        let expires_at = config
            .ttl_secs
            .map(|ttl| now + chrono::Duration::seconds(ttl as i64));

        Self {
            id,
            parent_id,
            session_type,
            tenant_id: None,
            mode: config.mode.clone(),
            state: SessionState::Created,
            permission_policy: config.permission_policy.clone(),
            config,
            messages: Vec::with_capacity(32),
            current_leaf_id: None,
            summary: None,
            total_usage: TokenUsage::default(),
            current_input_tokens: 0,
            total_cost_usd: 0.0,
            static_context_hash: None,
            created_at: now,
            updated_at: now,
            expires_at,
            error: None,
            todos: Vec::with_capacity(8),
            current_plan: None,
            compact_history: Vec::new(),
        }
    }

    pub fn is_subagent(&self) -> bool {
        matches!(self.session_type, SessionType::Subagent { .. })
    }

    pub fn is_running(&self) -> bool {
        matches!(
            self.state,
            SessionState::Active | SessionState::WaitingForTools
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            SessionState::Completed | SessionState::Failed | SessionState::Cancelled
        )
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expires| Utc::now() > expires)
    }

    pub fn add_message(&mut self, mut message: SessionMessage) {
        if let Some(leaf) = &self.current_leaf_id {
            message.parent_id = Some(leaf.clone());
        }
        self.current_leaf_id = Some(message.id.clone());
        if let Some(usage) = &message.usage {
            self.total_usage.add(usage);
        }
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    pub fn get_current_branch(&self) -> Vec<&SessionMessage> {
        let index: HashMap<&MessageId, &SessionMessage> =
            self.messages.iter().map(|m| (&m.id, m)).collect();

        let mut result = Vec::new();
        let mut current_id = self.current_leaf_id.as_ref();

        while let Some(id) = current_id {
            if let Some(&msg) = index.get(id) {
                result.push(msg);
                current_id = msg.parent_id.as_ref();
            } else {
                break;
            }
        }

        result.reverse();
        result
    }

    /// Convert session messages to API format with default caching (5m TTL).
    pub fn to_api_messages(&self) -> Vec<Message> {
        self.to_api_messages_with_cache(Some(CacheTtl::FiveMinutes))
    }

    /// Convert session messages to API format with optional caching.
    ///
    /// Per Anthropic best practices, caches the last user message with the specified TTL.
    /// Pass `None` to disable caching.
    pub fn to_api_messages_with_cache(&self, ttl: Option<CacheTtl>) -> Vec<Message> {
        let branch = self.get_current_branch();
        if branch.is_empty() {
            return Vec::new();
        }

        let mut messages: Vec<Message> = branch.iter().map(|m| m.to_api_message()).collect();

        if let Some(ttl) = ttl {
            self.apply_cache_breakpoint(&mut messages, ttl);
        }

        messages
    }

    /// Apply cache breakpoint to the last user message.
    ///
    /// Per Anthropic best practices for multi-turn conversations,
    /// only the last user message needs cache_control to enable
    /// caching of the entire conversation history before it.
    fn apply_cache_breakpoint(&self, messages: &mut [Message], ttl: CacheTtl) {
        let last_user_idx = messages
            .iter()
            .enumerate()
            .rev()
            .find(|(_, m)| m.role == Role::User)
            .map(|(i, _)| i);

        if let Some(idx) = last_user_idx {
            messages[idx].set_cache_on_last_block(CacheControl::ephemeral().with_ttl(ttl));
        }
    }

    pub fn branch_length(&self) -> usize {
        self.get_current_branch().len()
    }

    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
        self.updated_at = Utc::now();
    }

    pub fn set_todos(&mut self, todos: Vec<TodoItem>) {
        self.todos = todos;
        self.updated_at = Utc::now();
    }

    pub fn todos_in_progress_count(&self) -> usize {
        self.todos
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count()
    }

    pub fn enter_plan_mode(&mut self, name: Option<String>) -> &Plan {
        let mut plan = Plan::new(self.id);
        if let Some(n) = name {
            plan = plan.with_name(n);
        }
        self.current_plan = Some(plan);
        self.updated_at = Utc::now();
        self.current_plan.as_ref().expect("plan was just set")
    }

    pub fn update_plan_content(&mut self, content: String) {
        if let Some(ref mut plan) = self.current_plan {
            plan.content = content;
            self.updated_at = Utc::now();
        }
    }

    pub fn exit_plan_mode(&mut self) -> Option<Plan> {
        if let Some(ref mut plan) = self.current_plan {
            plan.approve();
            self.updated_at = Utc::now();
        }
        self.current_plan.take()
    }

    pub fn cancel_plan(&mut self) -> Option<Plan> {
        if let Some(ref mut plan) = self.current_plan {
            plan.cancel();
            self.updated_at = Utc::now();
        }
        self.current_plan.take()
    }

    pub fn is_in_plan_mode(&self) -> bool {
        self.current_plan
            .as_ref()
            .is_some_and(|p| !p.status.is_terminal())
    }

    pub fn record_compact(&mut self, record: CompactRecord) {
        if self.compact_history.len() >= MAX_COMPACT_HISTORY_SIZE {
            self.compact_history.remove(0);
        }
        self.compact_history.push(record);
        self.updated_at = Utc::now();
    }

    pub fn update_summary(&mut self, summary: impl Into<String>) {
        self.summary = Some(summary.into());
        self.updated_at = Utc::now();
    }

    pub fn add_user_message(&mut self, content: impl Into<String>) {
        let msg = SessionMessage::user(vec![ContentBlock::text(content.into())]);
        self.add_message(msg);
    }

    pub fn add_assistant_message(&mut self, content: Vec<ContentBlock>, usage: Option<Usage>) {
        let mut msg = SessionMessage::assistant(content);
        if let Some(u) = usage {
            msg = msg.with_usage(TokenUsage {
                input_tokens: u.input_tokens as u64,
                output_tokens: u.output_tokens as u64,
                cache_read_input_tokens: u.cache_read_input_tokens.unwrap_or(0) as u64,
                cache_creation_input_tokens: u.cache_creation_input_tokens.unwrap_or(0) as u64,
            });
        }
        self.add_message(msg);
    }

    pub fn add_tool_results(&mut self, results: Vec<crate::types::ToolResultBlock>) {
        let content: Vec<ContentBlock> =
            results.into_iter().map(ContentBlock::ToolResult).collect();
        let msg = SessionMessage::user(content);
        self.add_message(msg);
    }

    pub fn current_tokens(&self) -> u64 {
        self.current_input_tokens
    }

    pub fn should_compact(&self, max_tokens: u64, threshold: f32, keep_messages: usize) -> bool {
        self.messages.len() > keep_messages
            && self.current_input_tokens as f32 > max_tokens as f32 * threshold
    }

    pub fn update_usage(&mut self, usage: &Usage) {
        self.current_input_tokens = usage.input_tokens as u64;
        self.total_usage.input_tokens += usage.input_tokens as u64;
        self.total_usage.output_tokens += usage.output_tokens as u64;
        if let Some(cache_read) = usage.cache_read_input_tokens {
            self.total_usage.cache_read_input_tokens += cache_read as u64;
        }
        if let Some(cache_creation) = usage.cache_creation_input_tokens {
            self.total_usage.cache_creation_input_tokens += cache_creation as u64;
        }
    }

    pub async fn compact(
        &mut self,
        client: &crate::Client,
        keep_messages: usize,
    ) -> crate::Result<crate::types::CompactResult> {
        use crate::client::ModelType;
        use crate::client::messages::CreateMessageRequest;
        use crate::types::CompactResult;

        if self.messages.len() <= keep_messages {
            return Ok(CompactResult::NotNeeded);
        }

        let tokens_before = self.current_input_tokens;
        let split_point = self.messages.len() - keep_messages;
        let to_summarize: Vec<_> = self.messages[..split_point].to_vec();
        let to_keep: Vec<_> = self.messages[split_point..].to_vec();

        let summary_prompt = Self::format_for_summary(&to_summarize);
        let model = client.adapter().model(ModelType::Small).to_string();
        let request = CreateMessageRequest::new(&model, vec![Message::user(&summary_prompt)])
            .with_max_tokens(2000);
        let response = client.send(request).await?;
        let summary = response.text();

        let original_count = self.messages.len();

        self.messages.clear();
        self.current_leaf_id = None;

        let summary_msg = SessionMessage::user(vec![ContentBlock::text(format!(
            "[Previous conversation summary]\n{}",
            summary
        ))])
        .as_compact_summary();
        self.add_message(summary_msg);

        for mut msg in to_keep {
            msg.parent_id = self.current_leaf_id.clone();
            self.current_leaf_id = Some(msg.id.clone());
            self.messages.push(msg);
        }

        // Reset to 0: actual value will be set by next API call's update_usage().
        // This also prevents immediate re-compaction since should_compact() returns false when 0.
        self.current_input_tokens = 0;
        self.summary = Some(summary.clone());
        self.updated_at = Utc::now();

        let record = CompactRecord::new(self.id)
            .with_counts(original_count, self.messages.len())
            .with_summary(summary.clone())
            .with_saved_tokens(tokens_before as usize);
        self.record_compact(record);

        Ok(CompactResult::Compacted {
            original_count,
            new_count: self.messages.len(),
            saved_tokens: tokens_before as usize,
            summary,
        })
    }

    fn format_for_summary(messages: &[SessionMessage]) -> String {
        let estimated_capacity = messages.len() * 500 + 200;
        let mut formatted = String::with_capacity(estimated_capacity.min(32768));
        formatted.push_str(
            "Summarize this conversation concisely. \
             Preserve key decisions, code changes, file paths, and important context:\n\n",
        );

        for msg in messages {
            let role = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            };
            formatted.push_str(role);
            formatted.push_str(":\n");

            for block in &msg.content {
                if let Some(text) = block.as_text() {
                    if text.len() > 800 {
                        formatted.push_str(&text[..800]);
                        formatted.push_str("... [truncated]\n");
                    } else {
                        formatted.push_str(text);
                        formatted.push('\n');
                    }
                }
            }
            formatted.push('\n');
        }

        formatted
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.current_leaf_id = None;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentBlock, Role};

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        let session = Session::new(config);

        assert_eq!(session.state, SessionState::Created);
        assert!(session.messages.is_empty());
        assert!(session.current_leaf_id.is_none());
    }

    #[test]
    fn test_add_message() {
        let mut session = Session::new(SessionConfig::default());

        let msg1 = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        session.add_message(msg1);

        assert_eq!(session.messages.len(), 1);
        assert!(session.current_leaf_id.is_some());
    }

    #[test]
    fn test_message_tree() {
        let mut session = Session::new(SessionConfig::default());

        let user_msg = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        session.add_message(user_msg);

        let assistant_msg = SessionMessage::assistant(vec![ContentBlock::text("Hi there!")]);
        session.add_message(assistant_msg);

        let branch = session.get_current_branch();
        assert_eq!(branch.len(), 2);
        assert_eq!(branch[0].role, Role::User);
        assert_eq!(branch[1].role, Role::Assistant);
    }

    #[test]
    fn test_session_expiry() {
        let config = SessionConfig {
            ttl_secs: Some(0),
            ..Default::default()
        };
        let session = Session::new(config);

        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(session.is_expired());
    }

    #[test]
    fn test_token_usage_accumulation() {
        let mut session = Session::new(SessionConfig::default());

        let msg1 = SessionMessage::assistant(vec![ContentBlock::text("Response 1")]).with_usage(
            TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
        );
        session.add_message(msg1);

        let msg2 = SessionMessage::assistant(vec![ContentBlock::text("Response 2")]).with_usage(
            TokenUsage {
                input_tokens: 150,
                output_tokens: 75,
                ..Default::default()
            },
        );
        session.add_message(msg2);

        assert_eq!(session.total_usage.input_tokens, 250);
        assert_eq!(session.total_usage.output_tokens, 125);
    }

    #[test]
    fn test_compact_history_limit() {
        let mut session = Session::new(SessionConfig::default());

        for i in 0..MAX_COMPACT_HISTORY_SIZE + 10 {
            let record = CompactRecord::new(session.id).with_summary(format!("Summary {}", i));
            session.record_compact(record);
        }

        assert_eq!(session.compact_history.len(), MAX_COMPACT_HISTORY_SIZE);
        assert!(session.compact_history[0].summary.contains("10"));
    }

    #[test]
    fn test_exit_plan_mode_takes_ownership() {
        let mut session = Session::new(SessionConfig::default());
        session.enter_plan_mode(Some("Test Plan".to_string()));

        let plan = session.exit_plan_mode();
        assert!(plan.is_some());
        assert!(session.current_plan.is_none());
    }

    #[test]
    fn test_message_caching_applies_to_last_user_turn() {
        let mut session = Session::new(SessionConfig::default());

        session.add_user_message("First question");
        session.add_message(SessionMessage::assistant(vec![ContentBlock::text(
            "First answer",
        )]));
        session.add_user_message("Second question");

        let messages = session.to_api_messages();

        assert_eq!(messages.len(), 3);
        assert!(!messages[0].has_cache_control());
        assert!(!messages[1].has_cache_control());
        assert!(messages[2].has_cache_control());
    }

    #[test]
    fn test_message_caching_disabled() {
        let mut session = Session::new(SessionConfig::default());

        session.add_user_message("Question");

        // Pass None to disable caching
        let messages = session.to_api_messages_with_cache(None);

        assert_eq!(messages.len(), 1);
        assert!(!messages[0].has_cache_control());
    }

    #[test]
    fn test_message_caching_empty_session() {
        let session = Session::new(SessionConfig::default());
        let messages = session.to_api_messages();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_message_caching_assistant_only() {
        let mut session = Session::new(SessionConfig::default());
        session.add_message(SessionMessage::assistant(vec![ContentBlock::text("Hi")]));

        let messages = session.to_api_messages();

        assert_eq!(messages.len(), 1);
        assert!(!messages[0].has_cache_control());
    }
}
