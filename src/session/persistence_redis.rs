//! Redis persistence backend for sessions.
//!
//! Enable with the `redis-backend` feature flag.

use async_trait::async_trait;
use redis::AsyncCommands;
use std::sync::Arc;
use std::time::Duration;

use super::persistence::Persistence;
use super::state::{Session, SessionId, SessionMessage};
use super::{SessionError, SessionResult};

/// Redis persistence backend.
pub struct RedisPersistence {
    client: Arc<redis::Client>,
    key_prefix: String,
    default_ttl: Option<Duration>,
}

impl RedisPersistence {
    /// Create a new Redis persistence backend.
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self {
            client: Arc::new(client),
            key_prefix: "claude:session:".to_string(),
            default_ttl: Some(Duration::from_secs(86400 * 7)), // 7 days
        })
    }

    /// Set custom key prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    /// Set default TTL for sessions.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    /// Disable TTL (sessions never expire in Redis).
    pub fn without_ttl(mut self) -> Self {
        self.default_ttl = None;
        self
    }

    fn session_key(&self, id: &SessionId) -> String {
        format!("{}{}", self.key_prefix, id)
    }

    fn tenant_key(&self, tenant_id: &str) -> String {
        format!("{}tenant:{}", self.key_prefix, tenant_id)
    }

    fn children_key(&self, parent_id: &SessionId) -> String {
        format!("{}children:{}", self.key_prefix, parent_id)
    }

    async fn get_connection(&self) -> SessionResult<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })
    }
}

#[async_trait]
impl Persistence for RedisPersistence {
    fn name(&self) -> &str {
        "redis"
    }

    async fn save(&self, session: &Session) -> SessionResult<()> {
        let mut conn = self.get_connection().await?;
        let key = self.session_key(&session.id);

        let data = serde_json::to_string(session).map_err(SessionError::Serialization)?;

        // Calculate TTL
        let ttl_secs = session
            .config
            .ttl_secs
            .or_else(|| self.default_ttl.map(|d| d.as_secs()));

        match ttl_secs {
            Some(ttl) => {
                conn.set_ex::<_, _, ()>(&key, &data, ttl)
                    .await
                    .map_err(|e| SessionError::Storage {
                        message: e.to_string(),
                    })?;
            }
            None => {
                conn.set::<_, _, ()>(&key, &data)
                    .await
                    .map_err(|e| SessionError::Storage {
                        message: e.to_string(),
                    })?;
            }
        }

        // Add to tenant set if tenant_id exists
        if let Some(ref tenant_id) = session.tenant_id {
            let tenant_key = self.tenant_key(tenant_id);
            conn.sadd::<_, _, ()>(&tenant_key, session.id.to_string())
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;
        }

        // Add to parent's children set if parent_id exists
        if let Some(parent_id) = session.parent_id {
            let children_key = self.children_key(&parent_id);
            conn.sadd::<_, _, ()>(&children_key, session.id.to_string())
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;
        }

        Ok(())
    }

    async fn load(&self, id: &SessionId) -> SessionResult<Option<Session>> {
        let mut conn = self.get_connection().await?;
        let key = self.session_key(id);

        let data: Option<String> = conn.get(&key).await.map_err(|e| SessionError::Storage {
            message: e.to_string(),
        })?;

        match data {
            Some(json) => {
                let session: Session =
                    serde_json::from_str(&json).map_err(SessionError::Serialization)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn delete(&self, id: &SessionId) -> SessionResult<bool> {
        let mut conn = self.get_connection().await?;
        let key = self.session_key(id);

        // First load to get tenant_id for cleanup
        if let Some(session) = self.load(id).await?
            && let Some(ref tenant_id) = session.tenant_id
        {
            let tenant_key = self.tenant_key(tenant_id);
            conn.srem::<_, _, ()>(&tenant_key, id.to_string())
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;
        }

        let deleted: i32 = conn.del(&key).await.map_err(|e| SessionError::Storage {
            message: e.to_string(),
        })?;

        Ok(deleted > 0)
    }

    async fn list(&self, tenant_id: Option<&str>) -> SessionResult<Vec<SessionId>> {
        let mut conn = self.get_connection().await?;

        match tenant_id {
            Some(tid) => {
                let tenant_key = self.tenant_key(tid);
                let ids: Vec<String> =
                    conn.smembers(&tenant_key)
                        .await
                        .map_err(|e| SessionError::Storage {
                            message: e.to_string(),
                        })?;
                Ok(ids.into_iter().map(SessionId::from).collect())
            }
            None => {
                // Scan all session keys
                let pattern = format!("{}*", self.key_prefix);
                let mut cursor: u64 = 0;
                let mut all_ids = Vec::new();

                loop {
                    let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(&pattern)
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut conn)
                        .await
                        .map_err(|e| SessionError::Storage {
                            message: e.to_string(),
                        })?;

                    for key in keys {
                        if let Some(id) = key.strip_prefix(&self.key_prefix)
                            && !id.starts_with("tenant:")
                        {
                            all_ids.push(SessionId::from(id));
                        }
                    }

                    cursor = next_cursor;
                    if cursor == 0 {
                        break;
                    }
                }

                Ok(all_ids)
            }
        }
    }

    async fn list_children(&self, parent_id: &SessionId) -> SessionResult<Vec<SessionId>> {
        let mut conn = self.get_connection().await?;
        let children_key = self.children_key(parent_id);

        let ids: Vec<String> =
            conn.smembers(&children_key)
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;

        Ok(ids.into_iter().map(SessionId::from).collect())
    }

    async fn add_message(
        &self,
        session_id: &SessionId,
        message: SessionMessage,
    ) -> SessionResult<()> {
        let mut session = self
            .load(session_id)
            .await?
            .ok_or_else(|| SessionError::NotFound {
                id: session_id.to_string(),
            })?;

        session.add_message(message);
        self.save(&session).await
    }

    async fn cleanup_expired(&self) -> SessionResult<usize> {
        // Redis handles TTL-based expiration automatically
        // This is a no-op for Redis, but we return 0 for consistency
        Ok(0)
    }
}
