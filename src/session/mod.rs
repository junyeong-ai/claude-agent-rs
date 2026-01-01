//! Session management for stateful conversations.

pub mod cache;
pub mod compact;
pub mod manager;
pub mod persistence;
pub mod state;

// Re-exports
pub use crate::types::TokenUsage;
pub use cache::{CacheConfigBuilder, CacheStats, SessionCacheManager};
pub use compact::{CompactExecutor, CompactStrategy};
pub use manager::SessionManager;
pub use persistence::{MemoryPersistence, Persistence};
pub use state::{
    MessageId, MessageMetadata, PermissionPolicy, Session, SessionConfig, SessionId,
    SessionMessage, SessionMode, SessionState,
};

use thiserror::Error;

/// Errors that can occur in session management
#[derive(Error, Debug)]
pub enum SessionError {
    /// Session not found
    #[error("Session not found: {id}")]
    NotFound {
        /// Session ID that was not found
        id: String,
    },

    /// Session has expired
    #[error("Session expired: {id}")]
    Expired {
        /// Expired session ID
        id: String,
    },

    /// Permission denied for operation
    #[error("Permission denied: {reason}")]
    PermissionDenied {
        /// Reason for denial
        reason: String,
    },

    /// Database/storage error
    #[error("Storage error: {message}")]
    Storage {
        /// Error message
        message: String,
    },

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Compact failed
    #[error("Compact error: {message}")]
    Compact {
        /// Error message
        message: String,
    },

    /// Context error
    #[error("Context error: {0}")]
    Context(#[from] crate::context::ContextError),
}

/// Result type for session operations
pub type SessionResult<T> = std::result::Result<T, SessionError>;

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
