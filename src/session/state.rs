//! Session State and Types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::{ContentBlock, Message, Role, TokenUsage};

/// Unique session identifier
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    /// Generate a new random session ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique message identifier (for tree structure)
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct MessageId(pub String);

impl MessageId {
    /// Generate a new random message ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// Session mode
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionMode {
    /// Stateless mode (single request)
    #[default]
    Stateless,

    /// Stateful mode (persistent)
    Stateful {
        /// Persistence strategy name
        persistence: String,
    },
}

/// Session state
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Newly created
    #[default]
    Created,
    /// Actively running
    Active,
    /// Waiting for tool results
    WaitingForTools,
    /// Waiting for user input
    WaitingForUser,
    /// Paused
    Paused,
    /// Completed successfully
    Completed,
    /// Error state
    Error,
}

/// Permission mode (non-interactive, Chat API compatible)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Default mode: allow/deny rules only (no ask)
    #[default]
    Default,
    /// Auto-approve file edits
    AcceptEdits,
    /// Bypass all permissions (dangerous)
    Bypass,
    /// Plan mode: read-only tools only
    Plan,
}

/// Permission policy for a session
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// Permission mode
    pub mode: PermissionMode,
    /// Explicitly allowed tool patterns
    #[serde(default)]
    pub allow: Vec<String>,
    /// Explicitly denied tool patterns
    #[serde(default)]
    pub deny: Vec<String>,
    /// Tool-specific limits
    #[serde(default)]
    pub tool_limits: HashMap<String, ToolLimits>,
}

/// Tool-specific limits
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ToolLimits {
    /// Timeout in milliseconds
    pub timeout_ms: Option<u64>,
    /// Maximum output size
    pub max_output_size: Option<usize>,
}

/// Session configuration (for creation)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Model to use
    pub model: String,
    /// Maximum tokens
    pub max_tokens: u32,
    /// Permission policy
    #[serde(default)]
    pub permission_policy: PermissionPolicy,
    /// Session mode
    #[serde(default)]
    pub mode: SessionMode,
    /// TTL in seconds (None = no expiry)
    pub ttl_secs: Option<u64>,
    /// System prompt override
    pub system_prompt: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-5".to_string(),
            max_tokens: 16384,
            permission_policy: PermissionPolicy::default(),
            mode: SessionMode::default(),
            ttl_secs: None,
            system_prompt: None,
        }
    }
}

/// Message metadata
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Model that generated this message
    pub model: Option<String>,
    /// API request ID
    pub request_id: Option<String>,
    /// Tool results (for user messages with tool_result)
    pub tool_results: Option<Vec<ToolResultMeta>>,
    /// Thinking metadata
    pub thinking: Option<ThinkingMetadata>,
}

/// Tool result metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResultMeta {
    /// Tool use ID
    pub tool_use_id: String,
    /// Tool name
    pub tool_name: String,
    /// Whether it was an error
    pub is_error: bool,
    /// Execution duration in ms
    pub duration_ms: Option<u64>,
}

/// Thinking (extended thinking) metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThinkingMetadata {
    /// Thinking level
    pub level: String,
    /// Whether thinking was disabled
    pub disabled: bool,
}

/// Session message (tree node)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Message ID
    pub id: MessageId,
    /// Parent message ID (None for root)
    pub parent_id: Option<MessageId>,
    /// Role
    pub role: Role,
    /// Content blocks
    pub content: Vec<ContentBlock>,
    /// Is this a side-chain (branch)?
    #[serde(default)]
    pub is_sidechain: bool,
    /// Token usage (from API response)
    pub usage: Option<TokenUsage>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Metadata
    #[serde(default)]
    pub metadata: MessageMetadata,
}

impl SessionMessage {
    /// Create a new user message
    pub fn user(content: Vec<ContentBlock>) -> Self {
        Self {
            id: MessageId::new(),
            parent_id: None,
            role: Role::User,
            content,
            is_sidechain: false,
            usage: None,
            timestamp: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    /// Create a new assistant message
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            id: MessageId::new(),
            parent_id: None,
            role: Role::Assistant,
            content,
            is_sidechain: false,
            usage: None,
            timestamp: Utc::now(),
            metadata: MessageMetadata::default(),
        }
    }

    /// Set parent ID
    pub fn with_parent(mut self, parent_id: MessageId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Set token usage
    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    /// Mark as sidechain
    pub fn as_sidechain(mut self) -> Self {
        self.is_sidechain = true;
        self
    }

    /// Convert to API Message format
    pub fn to_api_message(&self) -> Message {
        Message {
            role: self.role,
            content: self.content.clone(),
        }
    }
}

/// Full session state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    /// Session ID
    pub id: SessionId,
    /// Tenant ID (for multi-tenant)
    pub tenant_id: Option<String>,
    /// Session mode
    pub mode: SessionMode,
    /// Session state
    pub state: SessionState,
    /// Configuration
    pub config: SessionConfig,
    /// Permission policy
    pub permission_policy: PermissionPolicy,
    /// Messages (tree structure)
    pub messages: Vec<SessionMessage>,
    /// Current leaf message ID (active branch)
    pub current_leaf_id: Option<MessageId>,
    /// Conversation summary (after compact)
    pub summary: Option<String>,
    /// Cumulative token usage
    pub total_usage: TokenUsage,
    /// Cumulative cost (USD)
    pub total_cost_usd: f64,
    /// Static context hash (for Prompt Caching)
    pub static_context_hash: Option<String>,
    /// Created timestamp
    pub created_at: DateTime<Utc>,
    /// Updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Expiry timestamp
    pub expires_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Create a new session with config
    pub fn new(config: SessionConfig) -> Self {
        let now = Utc::now();
        let expires_at = config.ttl_secs.map(|ttl| now + chrono::Duration::seconds(ttl as i64));

        Self {
            id: SessionId::new(),
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
        }
    }

    /// Check if session has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            Utc::now() > expires
        } else {
            false
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, mut message: SessionMessage) {
        // Set parent to current leaf
        if let Some(leaf) = &self.current_leaf_id {
            message.parent_id = Some(leaf.clone());
        }

        // Update current leaf
        self.current_leaf_id = Some(message.id.clone());

        // Update usage
        if let Some(usage) = &message.usage {
            self.total_usage.add(usage);
        }

        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Get the current conversation branch (from root to leaf)
    pub fn get_current_branch(&self) -> Vec<&SessionMessage> {
        let mut result = Vec::new();
        let mut current_id = self.current_leaf_id.clone();

        while let Some(id) = current_id {
            if let Some(msg) = self.messages.iter().find(|m| m.id == id) {
                result.push(msg);
                current_id = msg.parent_id.clone();
            } else {
                break;
            }
        }

        result.reverse();
        result
    }

    /// Convert current branch to API messages
    pub fn to_api_messages(&self) -> Vec<Message> {
        self.get_current_branch()
            .into_iter()
            .map(|m| m.to_api_message())
            .collect()
    }

    /// Get message count in current branch
    pub fn branch_length(&self) -> usize {
        self.get_current_branch().len()
    }

    /// Update session state
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

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
            ttl_secs: Some(0), // Expire immediately
            ..Default::default()
        };
        let session = Session::new(config);

        // Should be expired (ttl=0)
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(session.is_expired());
    }

    #[test]
    fn test_token_usage_accumulation() {
        let mut session = Session::new(SessionConfig::default());

        let msg1 = SessionMessage::assistant(vec![ContentBlock::text("Response 1")])
            .with_usage(TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            });
        session.add_message(msg1);

        let msg2 = SessionMessage::assistant(vec![ContentBlock::text("Response 2")])
            .with_usage(TokenUsage {
                input_tokens: 150,
                output_tokens: 75,
                ..Default::default()
            });
        session.add_message(msg2);

        assert_eq!(session.total_usage.input_tokens, 250);
        assert_eq!(session.total_usage.output_tokens, 125);
    }
}
