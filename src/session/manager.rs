//! Session lifecycle management.

use std::sync::Arc;

use super::persistence::{MemoryPersistence, Persistence};
use super::state::{Session, SessionConfig, SessionId, SessionMessage, SessionState};
use super::{SessionError, SessionResult};

pub struct SessionManager {
    persistence: Arc<dyn Persistence>,
}

impl SessionManager {
    pub fn new(persistence: Arc<dyn Persistence>) -> Self {
        Self { persistence }
    }

    pub fn in_memory() -> Self {
        Self::new(Arc::new(MemoryPersistence::new()))
    }

    pub async fn create(&self, config: SessionConfig) -> SessionResult<Session> {
        let session = Session::new(config);
        self.persistence.save(&session).await?;
        Ok(session)
    }

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

    pub async fn get(&self, id: &SessionId) -> SessionResult<Session> {
        let session = self
            .persistence
            .load(id)
            .await?
            .ok_or_else(|| SessionError::NotFound { id: id.to_string() })?;

        if session.is_expired() {
            self.persistence.delete(id).await?;
            return Err(SessionError::Expired { id: id.to_string() });
        }

        Ok(session)
    }

    pub async fn get_by_str(&self, id: &str) -> SessionResult<Session> {
        self.get(&SessionId::from(id)).await
    }

    pub async fn update(&self, session: &Session) -> SessionResult<()> {
        self.persistence.save(session).await
    }

    pub async fn add_message(
        &self,
        session_id: &SessionId,
        message: SessionMessage,
    ) -> SessionResult<()> {
        self.persistence.add_message(session_id, message).await
    }

    pub async fn delete(&self, id: &SessionId) -> SessionResult<bool> {
        self.persistence.delete(id).await
    }

    pub async fn list(&self) -> SessionResult<Vec<SessionId>> {
        self.persistence.list(None).await
    }

    pub async fn list_for_tenant(&self, tenant_id: &str) -> SessionResult<Vec<SessionId>> {
        self.persistence.list(Some(tenant_id)).await
    }

    pub async fn fork(&self, id: &SessionId) -> SessionResult<Session> {
        let original = self.get(id).await?;

        let mut forked = Session::new(original.config.clone());
        forked.parent_id = Some(original.id);
        forked.tenant_id = original.tenant_id.clone();
        forked.summary = original.summary.clone();

        // Copy messages up to current leaf
        for msg in original.current_branch() {
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

    pub async fn complete(&self, id: &SessionId) -> SessionResult<()> {
        let mut session = self.get(id).await?;
        session.set_state(SessionState::Completed);
        self.persistence.save(&session).await
    }

    pub async fn set_error(&self, id: &SessionId) -> SessionResult<()> {
        let mut session = self.get(id).await?;
        session.set_state(SessionState::Failed);
        self.persistence.save(&session).await
    }

    pub async fn cleanup_expired(&self) -> SessionResult<usize> {
        self.persistence.cleanup_expired().await
    }

    pub async fn exists(&self, id: &SessionId) -> SessionResult<bool> {
        match self.persistence.load(id).await? {
            Some(session) => Ok(!session.is_expired()),
            None => Ok(false),
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::in_memory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ContentBlock;

    #[tokio::test]
    async fn test_session_manager_create() {
        let manager = SessionManager::in_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();

        assert_eq!(session.state, SessionState::Created);
        assert!(session.messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_manager_get() {
        let manager = SessionManager::in_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id;

        let restored = manager.get(&session_id).await.unwrap();
        assert_eq!(restored.id, session_id);
    }

    #[tokio::test]
    async fn test_session_manager_not_found() {
        let manager = SessionManager::in_memory();
        let fake_id = SessionId::new();

        let result = manager.get(&fake_id).await;
        assert!(matches!(result, Err(SessionError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_session_manager_add_message() {
        let manager = SessionManager::in_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id;

        let message = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        manager.add_message(&session_id, message).await.unwrap();

        let restored = manager.get(&session_id).await.unwrap();
        assert_eq!(restored.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_manager_fork() {
        let manager = SessionManager::in_memory();

        // Create original session with messages
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id;

        let msg1 = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        manager.add_message(&session_id, msg1).await.unwrap();

        let msg2 = SessionMessage::assistant(vec![ContentBlock::text("Hi!")]);
        manager.add_message(&session_id, msg2).await.unwrap();

        // Fork
        let forked = manager.fork(&session_id).await.unwrap();

        // Forked session should have the same messages
        assert_eq!(forked.messages.len(), 2);
        assert_ne!(forked.id, session_id);
        assert_eq!(forked.parent_id, Some(session_id));

        // Messages should be marked as sidechain
        assert!(forked.messages.iter().all(|m| m.is_sidechain));
    }

    #[tokio::test]
    async fn test_session_manager_complete() {
        let manager = SessionManager::in_memory();
        let session = manager.create(SessionConfig::default()).await.unwrap();
        let session_id = session.id;

        manager.complete(&session_id).await.unwrap();

        let completed = manager.get(&session_id).await.unwrap();
        assert_eq!(completed.state, SessionState::Completed);
    }

    #[tokio::test]
    async fn test_session_manager_tenant_filtering() {
        let manager = SessionManager::in_memory();

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
        let manager = SessionManager::in_memory();

        let config = SessionConfig {
            ttl_secs: Some(0), // Expire immediately
            ..Default::default()
        };
        let session = manager.create(config).await.unwrap();
        let session_id = session.id;

        // Wait for expiry
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let result = manager.get(&session_id).await;
        assert!(matches!(result, Err(SessionError::Expired { .. })));
    }
}
