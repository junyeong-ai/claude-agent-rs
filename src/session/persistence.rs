//! Session Persistence Backends
//!
//! Provides different storage backends for session persistence.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::state::{Session, SessionId, SessionMessage};
use super::{SessionError, SessionResult};

/// Trait for session persistence backends
#[async_trait::async_trait]
pub trait Persistence: Send + Sync {
    /// Backend name
    fn name(&self) -> &str;

    /// Save a session
    async fn save(&self, session: &Session) -> SessionResult<()>;

    /// Load a session by ID
    async fn load(&self, id: &SessionId) -> SessionResult<Option<Session>>;

    /// Delete a session
    async fn delete(&self, id: &SessionId) -> SessionResult<bool>;

    /// List session IDs (optionally filtered by tenant)
    async fn list(&self, tenant_id: Option<&str>) -> SessionResult<Vec<SessionId>>;

    /// Add a message to an existing session
    async fn add_message(&self, session_id: &SessionId, message: SessionMessage) -> SessionResult<()>;

    /// Clean up expired sessions
    async fn cleanup_expired(&self) -> SessionResult<usize>;
}

/// In-memory persistence (for testing and single-instance deployments)
#[derive(Debug, Default)]
pub struct MemoryPersistence {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl MemoryPersistence {
    /// Create a new memory persistence backend
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of stored sessions
    pub async fn count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Clear all sessions
    pub async fn clear(&self) {
        self.sessions.write().await.clear();
    }
}

#[async_trait::async_trait]
impl Persistence for MemoryPersistence {
    fn name(&self) -> &str {
        "memory"
    }

    async fn save(&self, session: &Session) -> SessionResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.0.clone(), session.clone());
        Ok(())
    }

    async fn load(&self, id: &SessionId) -> SessionResult<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(&id.0).cloned())
    }

    async fn delete(&self, id: &SessionId) -> SessionResult<bool> {
        let mut sessions = self.sessions.write().await;
        Ok(sessions.remove(&id.0).is_some())
    }

    async fn list(&self, tenant_id: Option<&str>) -> SessionResult<Vec<SessionId>> {
        let sessions = self.sessions.read().await;
        let ids: Vec<SessionId> = sessions
            .values()
            .filter(|s| {
                tenant_id
                    .map(|t| s.tenant_id.as_deref() == Some(t))
                    .unwrap_or(true)
            })
            .map(|s| s.id.clone())
            .collect();
        Ok(ids)
    }

    async fn add_message(&self, session_id: &SessionId, message: SessionMessage) -> SessionResult<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id.0) {
            session.add_message(message);
            Ok(())
        } else {
            Err(SessionError::NotFound {
                id: session_id.0.clone(),
            })
        }
    }

    async fn cleanup_expired(&self) -> SessionResult<usize> {
        let mut sessions = self.sessions.write().await;
        let before = sessions.len();

        sessions.retain(|_, s| !s.is_expired());

        Ok(before - sessions.len())
    }
}

/// Persistence factory for creating backends
pub struct PersistenceFactory;

impl PersistenceFactory {
    /// Create a memory persistence backend
    pub fn memory() -> Arc<dyn Persistence> {
        Arc::new(MemoryPersistence::new())
    }

    // Future: Add database and Redis backends
    // pub fn postgres(pool: PgPool) -> Arc<dyn Persistence> { ... }
    // pub fn redis(client: RedisClient) -> Arc<dyn Persistence> { ... }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::state::SessionConfig;
    use crate::types::ContentBlock;

    #[tokio::test]
    async fn test_memory_persistence_save_load() {
        let persistence = MemoryPersistence::new();

        let session = Session::new(SessionConfig::default());
        let session_id = session.id.clone();

        persistence.save(&session).await.unwrap();

        let loaded = persistence.load(&session_id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, session_id);
    }

    #[tokio::test]
    async fn test_memory_persistence_delete() {
        let persistence = MemoryPersistence::new();

        let session = Session::new(SessionConfig::default());
        let session_id = session.id.clone();

        persistence.save(&session).await.unwrap();
        assert!(persistence.delete(&session_id).await.unwrap());
        assert!(persistence.load(&session_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_memory_persistence_list() {
        let persistence = MemoryPersistence::new();

        let mut session1 = Session::new(SessionConfig::default());
        session1.tenant_id = Some("tenant-a".to_string());

        let mut session2 = Session::new(SessionConfig::default());
        session2.tenant_id = Some("tenant-b".to_string());

        persistence.save(&session1).await.unwrap();
        persistence.save(&session2).await.unwrap();

        // List all
        let all = persistence.list(None).await.unwrap();
        assert_eq!(all.len(), 2);

        // List by tenant
        let tenant_a = persistence.list(Some("tenant-a")).await.unwrap();
        assert_eq!(tenant_a.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_persistence_add_message() {
        let persistence = MemoryPersistence::new();

        let session = Session::new(SessionConfig::default());
        let session_id = session.id.clone();
        persistence.save(&session).await.unwrap();

        let message = crate::session::state::SessionMessage::user(vec![ContentBlock::text("Hello")]);
        persistence.add_message(&session_id, message).await.unwrap();

        let loaded = persistence.load(&session_id).await.unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_persistence_cleanup_expired() {
        let persistence = MemoryPersistence::new();

        // Create expired session
        let config = SessionConfig {
            ttl_secs: Some(0), // Expire immediately
            ..Default::default()
        };
        let session = Session::new(config);
        persistence.save(&session).await.unwrap();

        // Wait a bit for expiry
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let cleaned = persistence.cleanup_expired().await.unwrap();
        assert_eq!(cleaned, 1);
        assert_eq!(persistence.count().await, 0);
    }
}
