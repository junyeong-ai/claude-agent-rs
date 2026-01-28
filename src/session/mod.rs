//! Session management for stateful conversations.

pub mod compact;
pub mod manager;
pub mod persistence;
#[cfg(feature = "jsonl")]
pub mod persistence_jsonl;
#[cfg(feature = "postgres")]
pub mod persistence_postgres;
#[cfg(feature = "redis-backend")]
pub mod persistence_redis;
pub mod queue;
pub mod session_state;
pub mod state;
pub mod types;

pub use crate::types::TokenUsage;
pub use compact::{CompactExecutor, CompactStrategy};
pub use manager::SessionManager;
pub use persistence::{MemoryPersistence, Persistence, PersistenceFactory};
#[cfg(feature = "jsonl")]
pub use persistence_jsonl::{
    JsonlConfig, JsonlConfigBuilder, JsonlEntry, JsonlPersistence, SyncMode,
};
#[cfg(feature = "postgres")]
pub use persistence_postgres::{
    PgPoolConfig, PostgresConfig, PostgresPersistence, PostgresSchema, SchemaIssue,
};
#[cfg(feature = "redis-backend")]
pub use persistence_redis::{RedisConfig, RedisPersistence};
pub use queue::{InputQueue, MergedInput, QueueError, QueuedInput, SharedInputQueue};
pub use session_state::{ExecutionGuard, ToolState};
pub use state::{
    MessageId, MessageMetadata, PermissionPolicy, Session, SessionConfig, SessionId,
    SessionMessage, SessionMode, SessionState, SessionType,
};
pub use types::{
    CompactRecord, CompactTrigger, EnvironmentContext, Plan, PlanStatus, QueueItem, QueueOperation,
    QueueStatus, SessionStats, SessionTree, SummarySnapshot, TodoItem, TodoStatus, ToolExecution,
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {id}")]
    NotFound { id: String },

    #[error("Session expired: {id}")]
    Expired { id: String },

    #[error("Permission denied: {reason}")]
    PermissionDenied { reason: String },

    #[error("Storage error: {message}")]
    Storage { message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Compact error: {message}")]
    Compact { message: String },

    #[error("Context error: {0}")]
    Context(#[from] crate::context::ContextError),

    #[error("Plan error: {message}")]
    Plan { message: String },
}

pub type SessionResult<T> = std::result::Result<T, SessionError>;

/// Extension trait for converting errors to SessionError::Storage.
pub trait StorageResultExt<T> {
    fn storage_err(self) -> SessionResult<T>;
    fn storage_err_ctx(self, context: &str) -> SessionResult<T>;
}

impl<T, E: std::fmt::Display> StorageResultExt<T> for std::result::Result<T, E> {
    fn storage_err(self) -> SessionResult<T> {
        self.map_err(|e| SessionError::Storage {
            message: e.to_string(),
        })
    }

    fn storage_err_ctx(self, context: &str) -> SessionResult<T> {
        self.map_err(|e| SessionError::Storage {
            message: format!("{}: {}", context, e),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_error_display() {
        let err = SessionError::NotFound {
            id: "test-123".to_string(),
        };
        assert!(err.to_string().contains("test-123"));
    }

    #[test]
    fn test_session_error_expired() {
        let err = SessionError::Expired {
            id: "sess-456".to_string(),
        };
        assert!(err.to_string().contains("expired"));
    }
}
