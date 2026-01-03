//! PostgreSQL persistence backend for sessions.
//!
//! Enable with the `postgres` feature flag.

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use std::sync::Arc;

use super::persistence::Persistence;
use super::state::{Session, SessionId, SessionMessage};
use super::{SessionError, SessionResult};

/// PostgreSQL persistence backend.
pub struct PostgresPersistence {
    pool: Arc<PgPool>,
    table_name: String,
}

impl PostgresPersistence {
    /// Create a new PostgreSQL persistence backend.
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;
        Self::with_pool(Arc::new(pool))
    }

    /// Create with an existing connection pool.
    pub fn with_pool(pool: Arc<PgPool>) -> Result<Self, sqlx::Error> {
        Ok(Self {
            pool,
            table_name: "claude_sessions".to_string(),
        })
    }

    /// Set custom table name.
    pub fn with_table_name(mut self, name: impl Into<String>) -> Self {
        self.table_name = name.into();
        self
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        let query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {} (
                id VARCHAR(255) PRIMARY KEY,
                parent_id VARCHAR(255),
                tenant_id VARCHAR(255),
                data JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ
            );
            CREATE INDEX IF NOT EXISTS idx_{}_tenant ON {} (tenant_id);
            CREATE INDEX IF NOT EXISTS idx_{}_expires ON {} (expires_at);
            CREATE INDEX IF NOT EXISTS idx_{}_parent ON {} (parent_id);
            "#,
            self.table_name,
            self.table_name,
            self.table_name,
            self.table_name,
            self.table_name,
            self.table_name,
            self.table_name
        );
        sqlx::query(&query).execute(&*self.pool).await?;
        Ok(())
    }
}

#[async_trait]
impl Persistence for PostgresPersistence {
    fn name(&self) -> &str {
        "postgres"
    }

    async fn save(&self, session: &Session) -> SessionResult<()> {
        let data = serde_json::to_value(session).map_err(SessionError::Serialization)?;

        let query = format!(
            r#"
            INSERT INTO {} (id, parent_id, tenant_id, data, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (id) DO UPDATE SET
                data = $4,
                updated_at = NOW(),
                expires_at = $5
            "#,
            self.table_name
        );

        sqlx::query(&query)
            .bind(session.id.to_string())
            .bind(session.parent_id.map(|p| p.to_string()))
            .bind(&session.tenant_id)
            .bind(&data)
            .bind(session.expires_at)
            .execute(&*self.pool)
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        Ok(())
    }

    async fn load(&self, id: &SessionId) -> SessionResult<Option<Session>> {
        let query = format!(
            "SELECT data FROM {} WHERE id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
            self.table_name
        );

        let row = sqlx::query(&query)
            .bind(id.to_string())
            .fetch_optional(&*self.pool)
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        match row {
            Some(row) => {
                let data: serde_json::Value = row.get("data");
                let session: Session =
                    serde_json::from_value(data).map_err(SessionError::Serialization)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    async fn delete(&self, id: &SessionId) -> SessionResult<bool> {
        let query = format!("DELETE FROM {} WHERE id = $1", self.table_name);
        let result = sqlx::query(&query)
            .bind(id.to_string())
            .execute(&*self.pool)
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        Ok(result.rows_affected() > 0)
    }

    async fn list(&self, tenant_id: Option<&str>) -> SessionResult<Vec<SessionId>> {
        let query = match tenant_id {
            Some(_) => format!(
                "SELECT id FROM {} WHERE tenant_id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
                self.table_name
            ),
            None => format!(
                "SELECT id FROM {} WHERE expires_at IS NULL OR expires_at > NOW()",
                self.table_name
            ),
        };

        let rows = match tenant_id {
            Some(tid) => sqlx::query(&query).bind(tid).fetch_all(&*self.pool).await,
            None => sqlx::query(&query).fetch_all(&*self.pool).await,
        }
        .map_err(|e| SessionError::Storage {
            message: e.to_string(),
        })?;

        let ids = rows
            .iter()
            .map(|row| SessionId::from(row.get::<String, _>("id")))
            .collect();

        Ok(ids)
    }

    async fn list_children(&self, parent_id: &SessionId) -> SessionResult<Vec<SessionId>> {
        let query = format!(
            "SELECT id FROM {} WHERE parent_id = $1 AND (expires_at IS NULL OR expires_at > NOW())",
            self.table_name
        );

        let rows = sqlx::query(&query)
            .bind(parent_id.to_string())
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        let ids = rows
            .iter()
            .map(|row| SessionId::from(row.get::<String, _>("id")))
            .collect();

        Ok(ids)
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
        let query = format!(
            "DELETE FROM {} WHERE expires_at IS NOT NULL AND expires_at <= NOW()",
            self.table_name
        );
        let result = sqlx::query(&query)
            .execute(&*self.pool)
            .await
            .map_err(|e| SessionError::Storage {
                message: e.to_string(),
            })?;

        Ok(result.rows_affected() as usize)
    }
}
