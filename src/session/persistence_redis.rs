//! Redis persistence backend for sessions.

use async_trait::async_trait;
use redis::AsyncCommands;
use std::sync::Arc;
use std::time::Duration;

use super::persistence::Persistence;
use super::state::{Session, SessionId};
use super::types::{QueueItem, SummarySnapshot};
use super::{SessionError, SessionResult, StorageResultExt};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct RedisConfig {
    pub key_prefix: String,
    pub default_ttl: Option<Duration>,
    pub connection_timeout: Duration,
    pub response_timeout: Duration,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            key_prefix: "claude:session:".to_string(),
            default_ttl: Some(Duration::from_secs(86400 * 7)),
            connection_timeout: Duration::from_secs(10),
            response_timeout: Duration::from_secs(30),
        }
    }
}

impl RedisConfig {
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    pub fn without_ttl(mut self) -> Self {
        self.default_ttl = None;
        self
    }
}

pub struct RedisPersistence {
    client: Arc<redis::Client>,
    config: RedisConfig,
}

impl RedisPersistence {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        Self::from_config(redis_url, RedisConfig::default())
    }

    pub fn from_config(redis_url: &str, config: RedisConfig) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        Ok(Self {
            client: Arc::new(client),
            config,
        })
    }

    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.config.key_prefix = prefix.into();
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = Some(ttl);
        self
    }

    pub fn without_ttl(mut self) -> Self {
        self.config.default_ttl = None;
        self
    }

    fn session_key(&self, id: &SessionId) -> String {
        format!("{}{}", self.config.key_prefix, id)
    }

    fn tenant_key(&self, tenant_id: &str) -> String {
        format!("{}tenant:{}", self.config.key_prefix, tenant_id)
    }

    fn children_key(&self, parent_id: &SessionId) -> String {
        format!("{}children:{}", self.config.key_prefix, parent_id)
    }

    fn summaries_key(&self, session_id: &SessionId) -> String {
        format!("{}summaries:{}", self.config.key_prefix, session_id)
    }

    fn queue_key(&self, session_id: &SessionId) -> String {
        format!("{}queue:{}", self.config.key_prefix, session_id)
    }

    async fn get_connection(&self) -> SessionResult<redis::aio::MultiplexedConnection> {
        tokio::time::timeout(
            self.config.connection_timeout,
            self.client.get_multiplexed_async_connection(),
        )
        .await
        .map_err(|_| SessionError::Storage {
            message: "Redis connection timeout".into(),
        })?
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

        let ttl_secs = session
            .config
            .ttl_secs
            .or_else(|| self.config.default_ttl.map(|d| d.as_secs()));

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

        if let Some(ref tenant_id) = session.tenant_id {
            conn.sadd::<_, _, ()>(&self.tenant_key(tenant_id), session.id.to_string())
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;
        }

        if let Some(parent_id) = session.parent_id {
            conn.sadd::<_, _, ()>(&self.children_key(&parent_id), session.id.to_string())
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

        let data: Option<String> = conn.get(&key).await.storage_err()?;

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

        if let Some(session) = self.load(id).await?
            && let Some(ref tenant_id) = session.tenant_id
        {
            conn.srem::<_, _, ()>(&self.tenant_key(tenant_id), id.to_string())
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;
        }

        conn.del::<_, ()>(&self.summaries_key(id))
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;
        conn.del::<_, ()>(&self.queue_key(id))
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        let deleted: i32 = conn.del(&key).await.storage_err()?;

        Ok(deleted > 0)
    }

    async fn list(&self, tenant_id: Option<&str>) -> SessionResult<Vec<SessionId>> {
        let mut conn = self.get_connection().await?;

        match tenant_id {
            Some(tid) => {
                let ids: Vec<String> = conn.smembers(self.tenant_key(tid)).await.map_err(|e| {
                    SessionError::Storage {
                        message: e.to_string(),
                    }
                })?;
                Ok(ids.into_iter().map(SessionId::from).collect())
            }
            None => {
                let pattern = format!("{}*", self.config.key_prefix);
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
                        if let Some(id) = key.strip_prefix(&self.config.key_prefix)
                            && !id.contains(':')
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
        let ids: Vec<String> = conn
            .smembers(self.children_key(parent_id))
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;
        Ok(ids.into_iter().map(SessionId::from).collect())
    }

    async fn add_summary(&self, snapshot: SummarySnapshot) -> SessionResult<()> {
        let mut conn = self.get_connection().await?;
        let key = self.summaries_key(&snapshot.session_id);
        let data = serde_json::to_string(&snapshot).map_err(SessionError::Serialization)?;

        conn.rpush::<_, _, ()>(&key, &data)
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        Ok(())
    }

    async fn get_summaries(&self, session_id: &SessionId) -> SessionResult<Vec<SummarySnapshot>> {
        let mut conn = self.get_connection().await?;
        let key = self.summaries_key(session_id);

        let items: Vec<String> =
            conn.lrange(&key, 0, -1)
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;

        items
            .into_iter()
            .map(|json| serde_json::from_str(&json).map_err(SessionError::Serialization))
            .collect()
    }

    async fn enqueue(
        &self,
        session_id: &SessionId,
        content: String,
        priority: i32,
    ) -> SessionResult<QueueItem> {
        let mut conn = self.get_connection().await?;
        let key = self.queue_key(session_id);
        let item = QueueItem::enqueue(*session_id, &content).with_priority(priority);
        let data = serde_json::to_string(&item).map_err(SessionError::Serialization)?;

        conn.zadd::<_, _, _, ()>(&key, &data, -(priority as f64))
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        Ok(item)
    }

    async fn dequeue(&self, session_id: &SessionId) -> SessionResult<Option<QueueItem>> {
        let mut conn = self.get_connection().await?;
        let key = self.queue_key(session_id);

        let items: Vec<String> =
            conn.zpopmin(&key, 1)
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;

        if items.is_empty() {
            return Ok(None);
        }

        let json = &items[0];
        let mut item: QueueItem =
            serde_json::from_str(json).map_err(SessionError::Serialization)?;
        item.start_processing();
        Ok(Some(item))
    }

    async fn cancel_queued(&self, item_id: Uuid) -> SessionResult<bool> {
        let mut conn = self.get_connection().await?;
        let pattern = format!("{}queue:*", self.config.key_prefix);

        let mut cursor: u64 = 0;
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
                let items: Vec<String> =
                    conn.zrange(&key, 0, -1)
                        .await
                        .map_err(|e| SessionError::Storage {
                            message: e.to_string(),
                        })?;

                for json in items {
                    if let Ok(item) = serde_json::from_str::<QueueItem>(&json)
                        && item.id == item_id
                    {
                        let removed: i32 =
                            conn.zrem(&key, &json)
                                .await
                                .map_err(|e| SessionError::Storage {
                                    message: e.to_string(),
                                })?;
                        return Ok(removed > 0);
                    }
                }
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(false)
    }

    async fn pending_queue(&self, session_id: &SessionId) -> SessionResult<Vec<QueueItem>> {
        let mut conn = self.get_connection().await?;
        let key = self.queue_key(session_id);

        let items: Vec<String> =
            conn.zrange(&key, 0, -1)
                .await
                .map_err(|e| SessionError::Storage {
                    message: e.to_string(),
                })?;

        items
            .into_iter()
            .map(|json| serde_json::from_str(&json).map_err(SessionError::Serialization))
            .collect()
    }

    async fn cleanup_expired(&self) -> SessionResult<usize> {
        Ok(0)
    }
}
