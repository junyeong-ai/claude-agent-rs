//! Session Manager
//!
//! High-level API for session lifecycle management.

use std::sync::Arc;

use super::persistence::{MemoryPersistence, Persistence};
use super::state::{Session, SessionConfig, SessionId, SessionMessage, SessionState};
use super::{SessionError, SessionResult};

/// Session manager for creating, restoring, and managing sessions
pub struct SessionManager {
    /// Persistence backend
    persistence: Arc<dyn Persistence>,
}

impl SessionManager {
    /// Create a new session manager with the specified persistence backend
    pub fn new(persistence: Arc<dyn Persistence>) -> Self {
        Self { persistence }
    }

    /// Create a session manager with in-memory persistence
    pub fn new_memory() -> Self {
        Self::new(Arc::new(MemoryPersistence::new()))
    }

    /// Get the persistence backend name
    pub fn backend_name(&self) -> &str {
        self.persistence.name()
    }

    /// Create a new session
    pub async fn create(&self, config: SessionConfig) -> SessionResult<Session> {
        let session = Session::new(config);
        self.persistence.save(&session).await?;
        Ok(session)
    }

    /// Create a new session with a tenant ID
    pub async fn create_with_tenant(
        &self,
        config: SessionConfig,
        tenant_id: impl Into<String>,
    ) -> SessionResult<Session> {
        let mut session = Session::new(config);
        session.tenant_id = Some(tenant_id.into());
        self.persistence.save(&session).await?;
        Ok(session)
    }

    /// Get a session by ID
    pub async fn get(&self, id: &SessionId) -> SessionResult<Session> {
        let session = self
            .persistence
            .load(id)
            .await?
            .ok_or_else(|| SessionError::NotFound { id: id.0.clone() })?;

        if session.is_expired() {
            // Clean up expired session
            self.persistence.delete(id).await?;
            return Err(SessionError::Expired { id: id.0.clone() });
        }

        Ok(session)
    }

    /// Restore a session by ID (alias for get)
    pub async fn restore(&self, id: &SessionId) -> SessionResult<Session> {
        self.get(id).await
    }

    /// Restore a session by string ID
    pub async fn restore_by_str(&self, id: &str) -> SessionResult<Session> {
        self.get(&SessionId::from_string(id)).await
    }

    /// Update a session
    pub async fn update(&self, session: &Session) -> SessionResult<()> {
        self.persistence.save(session).await
    }

    /// Add a message to a session
    pub async fn add_message(
        &self,
        session_id: &SessionId,
        message: SessionMessage,
    ) -> SessionResult<()> {
        self.persistence.add_message(session_id, message).await
    }

    /// Delete a session
    pub async fn delete(&self, id: &SessionId) -> SessionResult<bool> {
        self.persistence.delete(id).await
    }

    /// List all session IDs
    pub async fn list(&self) -> SessionResult<Vec<SessionId>> {
        self.persistence.list(None).await
    }

    /// List session IDs for a tenant
    pub async fn list_for_tenant(&self, tenant_id: &str) -> SessionResult<Vec<SessionId>> {
        self.persistence.list(Some(tenant_id)).await
    }

    /// Fork a session (create a branch)
    pub async fn fork(&self, id: &SessionId) -> SessionResult<Session> {
        let original = self.get(id).await?;

        let mut forked = Session::new(original.config.clone());
        forked.tenant_id = original.tenant_id.clone();
        forked.summary = original.summary.clone();

        // Copy messages up to current leaf
        for msg in original.get_current_branch() {
            let mut cloned = msg.clone();
            cloned.is_sidechain = true;
            forked.messages.push(cloned);
        }

        // Update leaf pointer
        if let Some(last) = forked.messages.last() {
            forked.current_leaf_id = Some(last.id.clone());
        }

        self.persistence.save(&forked).await?;
        Ok(forked)
    }

    /// Mark a session as completed
    pub async fn complete(&self, id: &SessionId) -> SessionResult<()> {
        let mut session = self.get(id).await?;
        session.set_state(SessionState::Completed);
        self.persistence.save(&session).await
    }

    /// Mark a session as errored
    pub async fn set_error(&self, id: &SessionId) -> SessionResult<()> {
        let mut session = self.get(id).await?;
        session.set_state(SessionState::Error);
        self.persistence.save(&session).await
    }

    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) -> SessionResult<usize> {
        self.persistence.cleanup_expired().await
    }

    /// Check if a session exists
    pub async fn exists(&self, id: &SessionId) -> SessionResult<bool> {
        match self.persistence.load(id).await? {
            Some(session) => Ok(!session.is_expired()),
            None => Ok(false),
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new_memory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ContentBlock;

    #[tokio::test]
    async fn test_session_manager_create() {
        let manager = SessionManager::new_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();

        assert_eq!(session.state, SessionState::Created);
        assert!(session.messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_manager_restore() {
        let manager = SessionManager::new_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id.clone();

        let restored = manager.restore(&session_id).await.unwrap();
        assert_eq!(restored.id, session_id);
    }

    #[tokio::test]
    async fn test_session_manager_not_found() {
        let manager = SessionManager::new_memory();
        let fake_id = SessionId::new();

        let result = manager.get(&fake_id).await;
        assert!(matches!(result, Err(SessionError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_session_manager_add_message() {
        let manager = SessionManager::new_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id.clone();

        let message = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        manager.add_message(&session_id, message).await.unwrap();

        let restored = manager.get(&session_id).await.unwrap();
        assert_eq!(restored.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_manager_fork() {
        let manager = SessionManager::new_memory();

        // Create original session with messages
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id.clone();

        let msg1 = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        manager.add_message(&session_id, msg1).await.unwrap();

        let msg2 = SessionMessage::assistant(vec![ContentBlock::text("Hi!")]);
        manager.add_message(&session_id, msg2).await.unwrap();

        // Fork
        let forked = manager.fork(&session_id).await.unwrap();

        // Forked session should have the same messages
        assert_eq!(forked.messages.len(), 2);
        assert_ne!(forked.id, session_id);

        // Messages should be marked as sidechain
        assert!(forked.messages.iter().all(|m| m.is_sidechain));
    }

    #[tokio::test]
    async fn test_session_manager_complete() {
        let manager = SessionManager::new_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id.clone();

        manager.complete(&session_id).await.unwrap();

        let completed = manager.get(&session_id).await.unwrap();
        assert_eq!(completed.state, SessionState::Completed);
    }

    #[tokio::test]
    async fn test_session_manager_tenant_filtering() {
        let manager = SessionManager::new_memory();

        let _s1 = manager
            .create_with_tenant(SessionConfig::default(), "tenant-a")
            .await
            .unwrap();
        let _s2 = manager
            .create_with_tenant(SessionConfig::default(), "tenant-a")
            .await
            .unwrap();
        let _s3 = manager
            .create_with_tenant(SessionConfig::default(), "tenant-b")
            .await
            .unwrap();

        let all = manager.list().await.unwrap();
        assert_eq!(all.len(), 3);

        let tenant_a = manager.list_for_tenant("tenant-a").await.unwrap();
        assert_eq!(tenant_a.len(), 2);

        let tenant_b = manager.list_for_tenant("tenant-b").await.unwrap();
        assert_eq!(tenant_b.len(), 1);
    }

    #[tokio::test]
    async fn test_session_manager_expired() {
        let manager = SessionManager::new_memory();

        let config = SessionConfig {
            ttl_secs: Some(0), // Expire immediately
            ..Default::default()
        };
        let session = manager.create(config).await.unwrap();
        let session_id = session.id.clone();

        // Wait for expiry
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let result = manager.get(&session_id).await;
        assert!(matches!(result, Err(SessionError::Expired { .. })));
    }
}
