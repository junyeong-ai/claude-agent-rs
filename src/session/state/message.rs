//! Session message types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::ids::MessageId;
use crate::types::{ContentBlock, Message, Role, TokenUsage};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub model: Option<String>,
    pub request_id: Option<String>,
    pub tool_results: Option<Vec<ToolResultMeta>>,
    pub thinking: Option<ThinkingMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResultMeta {
    pub tool_use_id: String,
    pub tool_name: String,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThinkingMetadata {
    pub level: String,
    pub disabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: MessageId,
    pub parent_id: Option<MessageId>,
    pub role: Role,
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub is_sidechain: bool,
    pub usage: Option<TokenUsage>,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub metadata: MessageMetadata,
}

impl SessionMessage {
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

    pub fn with_parent(mut self, parent_id: MessageId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }

    pub fn as_sidechain(mut self) -> Self {
        self.is_sidechain = true;
        self
    }

    pub fn to_api_message(&self) -> Message {
        Message {
            role: self.role,
            content: self.content.clone(),
        }
    }
}
