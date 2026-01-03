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
use crate::types::{Message, TokenUsage};

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
        let now = Utc::now();
        let expires_at = config
            .ttl_secs
            .map(|ttl| now + chrono::Duration::seconds(ttl as i64));

        Self {
            id,
            parent_id: None,
            session_type: SessionType::Main,
            tenant_id: None,
            mode: config.mode.clone(),
            state: SessionState::Created,
            config: config.clone(),
            permission_policy: config.permission_policy.clone(),
            messages: Vec::new(),
            current_leaf_id: None,
            summary: None,
            total_usage: TokenUsage::default(),
            total_cost_usd: 0.0,
            static_context_hash: None,
            created_at: now,
            updated_at: now,
            expires_at,
            error: None,
            todos: Vec::new(),
            current_plan: None,
            compact_history: Vec::new(),
        }
    }

    pub fn new_subagent(
        parent_id: SessionId,
        agent_type: impl Into<String>,
        description: impl Into<String>,
        config: SessionConfig,
    ) -> Self {
        let now = Utc::now();
        let expires_at = config
            .ttl_secs
            .map(|ttl| now + chrono::Duration::seconds(ttl as i64));

        Self {
            id: SessionId::new(),
            parent_id: Some(parent_id),
            session_type: SessionType::Subagent {
                agent_type: agent_type.into(),
                description: description.into(),
            },
            tenant_id: None,
            mode: config.mode.clone(),
            state: SessionState::Created,
            config: config.clone(),
            permission_policy: config.permission_policy.clone(),
            messages: Vec::new(),
            current_leaf_id: None,
            summary: None,
            total_usage: TokenUsage::default(),
            total_cost_usd: 0.0,
            static_context_hash: None,
            created_at: now,
            updated_at: now,
            expires_at,
            error: None,
            todos: Vec::new(),
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

    pub fn to_api_messages(&self) -> Vec<Message> {
        self.get_current_branch()
            .into_iter()
            .map(|m| m.to_api_message())
            .collect()
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
        self.current_plan.clone()
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
        self.compact_history.push(record);
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
}
