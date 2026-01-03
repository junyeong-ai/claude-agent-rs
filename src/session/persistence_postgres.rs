//! PostgreSQL session persistence with multi-table design.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::persistence::Persistence;
use super::state::{Session, SessionConfig, SessionId, SessionMessage};
use super::types::{CompactRecord, Plan, QueueItem, QueueStatus, SummarySnapshot, TodoItem};
use super::{SessionError, SessionResult};

#[derive(Clone, Debug)]
pub struct PostgresConfig {
    pub sessions_table: String,
    pub messages_table: String,
    pub compacts_table: String,
    pub summaries_table: String,
    pub queue_table: String,
    pub todos_table: String,
    pub plans_table: String,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self::with_prefix("claude_")
    }
}

impl PostgresConfig {
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            sessions_table: format!("{prefix}sessions"),
            messages_table: format!("{prefix}messages"),
            compacts_table: format!("{prefix}compacts"),
            summaries_table: format!("{prefix}summaries"),
            queue_table: format!("{prefix}queue"),
            todos_table: format!("{prefix}todos"),
            plans_table: format!("{prefix}plans"),
        }
    }
}

pub struct PostgresPersistence {
    pool: Arc<PgPool>,
    config: PostgresConfig,
}

impl PostgresPersistence {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;
        Self::with_pool(Arc::new(pool)).await
    }

    pub async fn with_pool(pool: Arc<PgPool>) -> Result<Self, sqlx::Error> {
        Self::with_config(pool, PostgresConfig::default()).await
    }

    pub async fn with_config(
        pool: Arc<PgPool>,
        config: PostgresConfig,
    ) -> Result<Self, sqlx::Error> {
        let persistence = Self { pool, config };
        persistence.migrate().await?;
        Ok(persistence)
    }

    pub async fn with_prefix(pool: Arc<PgPool>, prefix: &str) -> Result<Self, sqlx::Error> {
        Self::with_config(pool, PostgresConfig::with_prefix(prefix)).await
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        let c = &self.config;

        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {sessions} (
                id VARCHAR(255) PRIMARY KEY,
                parent_id VARCHAR(255),
                tenant_id VARCHAR(255),
                session_type VARCHAR(32) NOT NULL DEFAULT 'main',
                state VARCHAR(32) NOT NULL DEFAULT 'created',
                mode VARCHAR(32) NOT NULL DEFAULT 'default',
                config JSONB NOT NULL DEFAULT '{{}}',
                permission_policy JSONB NOT NULL DEFAULT '{{}}',
                summary TEXT,
                total_input_tokens BIGINT DEFAULT 0,
                total_output_tokens BIGINT DEFAULT 0,
                total_cost_usd DECIMAL(12, 6) DEFAULT 0,
                current_leaf_id VARCHAR(255),
                static_context_hash VARCHAR(64),
                error TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ,
                CONSTRAINT fk_{sessions}_parent FOREIGN KEY (parent_id) REFERENCES {sessions}(id) ON DELETE CASCADE
            )
            "#,
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {messages} (
                id VARCHAR(255) PRIMARY KEY,
                session_id VARCHAR(255) NOT NULL,
                parent_id VARCHAR(255),
                role VARCHAR(16) NOT NULL,
                content JSONB NOT NULL,
                is_sidechain BOOLEAN DEFAULT FALSE,
                is_compact_summary BOOLEAN DEFAULT FALSE,
                model VARCHAR(64),
                request_id VARCHAR(255),
                usage JSONB,
                metadata JSONB,
                environment JSONB,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT fk_{messages}_session FOREIGN KEY (session_id) REFERENCES {sessions}(id) ON DELETE CASCADE
            )
            "#,
            messages = c.messages_table,
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {compacts} (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                session_id VARCHAR(255) NOT NULL,
                trigger VARCHAR(32) NOT NULL,
                pre_tokens INTEGER NOT NULL,
                post_tokens INTEGER NOT NULL,
                saved_tokens INTEGER NOT NULL,
                summary TEXT NOT NULL,
                original_count INTEGER NOT NULL,
                new_count INTEGER NOT NULL,
                logical_parent_id VARCHAR(255),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT fk_{compacts}_session FOREIGN KEY (session_id) REFERENCES {sessions}(id) ON DELETE CASCADE
            )
            "#,
            compacts = c.compacts_table,
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {summaries} (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                session_id VARCHAR(255) NOT NULL,
                summary TEXT NOT NULL,
                leaf_message_id VARCHAR(255),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT fk_{summaries}_session FOREIGN KEY (session_id) REFERENCES {sessions}(id) ON DELETE CASCADE
            )
            "#,
            summaries = c.summaries_table,
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {queue} (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                session_id VARCHAR(255) NOT NULL,
                operation VARCHAR(32) NOT NULL,
                content TEXT NOT NULL,
                priority INTEGER DEFAULT 0,
                status VARCHAR(32) NOT NULL DEFAULT 'pending',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                processed_at TIMESTAMPTZ,
                CONSTRAINT fk_{queue}_session FOREIGN KEY (session_id) REFERENCES {sessions}(id) ON DELETE CASCADE
            )
            "#,
            queue = c.queue_table,
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {todos} (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                session_id VARCHAR(255) NOT NULL,
                plan_id UUID,
                content TEXT NOT NULL,
                active_form TEXT NOT NULL,
                status VARCHAR(32) NOT NULL DEFAULT 'pending',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                started_at TIMESTAMPTZ,
                completed_at TIMESTAMPTZ,
                CONSTRAINT fk_{todos}_session FOREIGN KEY (session_id) REFERENCES {sessions}(id) ON DELETE CASCADE
            )
            "#,
            todos = c.todos_table,
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await?;

        sqlx::query(&format!(
            r#"
            CREATE TABLE IF NOT EXISTS {plans} (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                session_id VARCHAR(255) NOT NULL,
                name VARCHAR(255),
                content TEXT NOT NULL,
                status VARCHAR(32) NOT NULL DEFAULT 'draft',
                error TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                approved_at TIMESTAMPTZ,
                started_at TIMESTAMPTZ,
                completed_at TIMESTAMPTZ,
                CONSTRAINT fk_{plans}_session FOREIGN KEY (session_id) REFERENCES {sessions}(id) ON DELETE CASCADE
            )
            "#,
            plans = c.plans_table,
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await?;

        let indexes = [
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_tenant ON {0}(tenant_id)",
                c.sessions_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_parent ON {0}(parent_id)",
                c.sessions_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_expires ON {0}(expires_at) WHERE expires_at IS NOT NULL",
                c.sessions_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_state ON {0}(state)",
                c.sessions_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_session ON {0}(session_id)",
                c.messages_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_created ON {0}(session_id, created_at)",
                c.messages_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_session ON {0}(session_id)",
                c.compacts_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_session ON {0}(session_id)",
                c.summaries_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_session_status ON {0}(session_id, status)",
                c.queue_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_session ON {0}(session_id)",
                c.todos_table
            ),
            format!(
                "CREATE INDEX IF NOT EXISTS idx_{0}_session ON {0}(session_id)",
                c.plans_table
            ),
        ];

        for sql in indexes {
            sqlx::query(&sql).execute(self.pool.as_ref()).await?;
        }

        Ok(())
    }

    async fn load_session_row(&self, session_id: &SessionId) -> SessionResult<Session> {
        let c = &self.config;
        let id_str = session_id.to_string();

        let row = sqlx::query(&format!(
            r#"
            SELECT id, parent_id, tenant_id, session_type, state, mode,
                   config, permission_policy, summary,
                   total_input_tokens, total_output_tokens, total_cost_usd,
                   current_leaf_id, static_context_hash, error,
                   created_at, updated_at, expires_at
            FROM {sessions}
            WHERE id = $1
            "#,
            sessions = c.sessions_table
        ))
        .bind(&id_str)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?
        .ok_or_else(|| SessionError::NotFound { id: id_str.clone() })?;

        let messages = self.load_messages(session_id).await?;
        let compacts = self.load_compacts(session_id).await?;
        let todos = self.load_todos_internal(session_id).await?;
        let plan = self.load_plan_internal(session_id).await?;

        let config: SessionConfig = row
            .try_get::<serde_json::Value, _>("config")
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let permission_policy = row
            .try_get::<serde_json::Value, _>("permission_policy")
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let session_type = row
            .try_get::<&str, _>("session_type")
            .ok()
            .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())
            .unwrap_or_default();

        let mode = row
            .try_get::<&str, _>("mode")
            .ok()
            .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())
            .unwrap_or_default();

        let state = row
            .try_get::<&str, _>("state")
            .ok()
            .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())
            .unwrap_or_default();

        let current_leaf_id = row
            .try_get::<&str, _>("current_leaf_id")
            .ok()
            .and_then(|s| s.parse().ok());

        Ok(Session {
            id: *session_id,
            parent_id: row
                .try_get::<&str, _>("parent_id")
                .ok()
                .and_then(|s| s.parse().ok()),
            session_type,
            tenant_id: row.try_get("tenant_id").ok(),
            mode,
            state,
            config,
            permission_policy,
            messages,
            current_leaf_id,
            summary: row.try_get("summary").ok(),
            total_usage: crate::types::TokenUsage {
                input_tokens: row.try_get::<i64, _>("total_input_tokens").unwrap_or(0) as u64,
                output_tokens: row.try_get::<i64, _>("total_output_tokens").unwrap_or(0) as u64,
                ..Default::default()
            },
            total_cost_usd: row
                .try_get::<rust_decimal::Decimal, _>("total_cost_usd")
                .ok()
                .and_then(|d| d.to_string().parse().ok())
                .unwrap_or(0.0),
            static_context_hash: row.try_get("static_context_hash").ok(),
            created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
            updated_at: row.try_get("updated_at").unwrap_or_else(|_| Utc::now()),
            expires_at: row.try_get("expires_at").ok(),
            error: row.try_get("error").ok(),
            todos,
            current_plan: plan,
            compact_history: compacts,
        })
    }

    async fn load_messages(&self, session_id: &SessionId) -> SessionResult<Vec<SessionMessage>> {
        let c = &self.config;

        let rows = sqlx::query(&format!(
            r#"
            SELECT id, parent_id, role, content, is_sidechain, is_compact_summary,
                   model, request_id, usage, metadata, environment, created_at
            FROM {messages}
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
            messages = c.messages_table
        ))
        .bind(session_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let content: Vec<crate::types::ContentBlock> = row
                    .try_get::<serde_json::Value, _>("content")
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())?;

                let role: crate::types::Role = row
                    .try_get::<&str, _>("role")
                    .ok()
                    .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())?;

                let usage = row
                    .try_get::<serde_json::Value, _>("usage")
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok());

                let metadata = row
                    .try_get::<serde_json::Value, _>("metadata")
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();

                let environment = row
                    .try_get::<serde_json::Value, _>("environment")
                    .ok()
                    .and_then(|v| serde_json::from_value(v).ok());

                Some(SessionMessage {
                    id: row.try_get::<&str, _>("id").ok()?.parse().ok()?,
                    parent_id: row
                        .try_get::<&str, _>("parent_id")
                        .ok()
                        .and_then(|s| s.parse().ok()),
                    role,
                    content,
                    is_sidechain: row.try_get("is_sidechain").unwrap_or(false),
                    is_compact_summary: row.try_get("is_compact_summary").unwrap_or(false),
                    usage,
                    timestamp: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                    metadata,
                    environment,
                })
            })
            .collect())
    }

    async fn load_compacts(&self, session_id: &SessionId) -> SessionResult<Vec<CompactRecord>> {
        let c = &self.config;

        let rows = sqlx::query(&format!(
            r#"
            SELECT id, session_id, trigger, pre_tokens, post_tokens, saved_tokens,
                   summary, original_count, new_count, logical_parent_id, created_at
            FROM {compacts}
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
            compacts = c.compacts_table
        ))
        .bind(session_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let trigger = row
                    .try_get::<&str, _>("trigger")
                    .ok()
                    .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())?;

                Some(CompactRecord {
                    id: row.try_get("id").ok()?,
                    session_id: row.try_get::<&str, _>("session_id").ok()?.parse().ok()?,
                    trigger,
                    pre_tokens: row.try_get::<i32, _>("pre_tokens").unwrap_or(0) as usize,
                    post_tokens: row.try_get::<i32, _>("post_tokens").unwrap_or(0) as usize,
                    saved_tokens: row.try_get::<i32, _>("saved_tokens").unwrap_or(0) as usize,
                    summary: row.try_get("summary").ok()?,
                    original_count: row.try_get::<i32, _>("original_count").unwrap_or(0) as usize,
                    new_count: row.try_get::<i32, _>("new_count").unwrap_or(0) as usize,
                    logical_parent_id: row
                        .try_get::<&str, _>("logical_parent_id")
                        .ok()
                        .and_then(|s| s.parse().ok()),
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                })
            })
            .collect())
    }

    async fn load_todos_internal(&self, session_id: &SessionId) -> SessionResult<Vec<TodoItem>> {
        let c = &self.config;

        let rows = sqlx::query(&format!(
            r#"
            SELECT id, session_id, plan_id, content, active_form, status,
                   created_at, started_at, completed_at
            FROM {todos}
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
            todos = c.todos_table
        ))
        .bind(session_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let status = row
                    .try_get::<&str, _>("status")
                    .ok()
                    .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())?;

                Some(TodoItem {
                    id: row.try_get("id").ok()?,
                    session_id: row.try_get::<&str, _>("session_id").ok()?.parse().ok()?,
                    content: row.try_get("content").ok()?,
                    active_form: row.try_get("active_form").ok()?,
                    status,
                    plan_id: row.try_get("plan_id").ok(),
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                    started_at: row.try_get("started_at").ok(),
                    completed_at: row.try_get("completed_at").ok(),
                })
            })
            .collect())
    }

    async fn load_plan_internal(&self, session_id: &SessionId) -> SessionResult<Option<Plan>> {
        let c = &self.config;

        let row = sqlx::query(&format!(
            r#"
            SELECT id, session_id, name, content, status, error,
                   created_at, approved_at, started_at, completed_at
            FROM {plans}
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            plans = c.plans_table
        ))
        .bind(session_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(row.and_then(|row| {
            let status = row
                .try_get::<&str, _>("status")
                .ok()
                .and_then(|s| serde_json::from_str(&format!("\"{}\"", s)).ok())?;

            Some(Plan {
                id: row.try_get("id").ok()?,
                session_id: row.try_get::<&str, _>("session_id").ok()?.parse().ok()?,
                name: row.try_get("name").ok(),
                content: row.try_get("content").ok()?,
                status,
                error: row.try_get("error").ok(),
                created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                approved_at: row.try_get("approved_at").ok(),
                started_at: row.try_get("started_at").ok(),
                completed_at: row.try_get("completed_at").ok(),
            })
        }))
    }

    async fn save_todos(&self, session_id: &SessionId, todos: &[TodoItem]) -> SessionResult<()> {
        let c = &self.config;

        sqlx::query(&format!(
            "DELETE FROM {todos} WHERE session_id = $1",
            todos = c.todos_table
        ))
        .bind(session_id.to_string())
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        for todo in todos {
            let status = serde_json::to_string(&todo.status)
                .unwrap_or_else(|_| "\"pending\"".to_string())
                .trim_matches('"')
                .to_string();

            sqlx::query(&format!(
                r#"
                INSERT INTO {todos} (
                    id, session_id, plan_id, content, active_form, status,
                    created_at, started_at, completed_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                todos = c.todos_table
            ))
            .bind(todo.id)
            .bind(session_id.to_string())
            .bind(todo.plan_id)
            .bind(&todo.content)
            .bind(&todo.active_form)
            .bind(&status)
            .bind(todo.created_at)
            .bind(todo.started_at)
            .bind(todo.completed_at)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| SessionError::PersistenceError(e.to_string()))?;
        }

        Ok(())
    }

    async fn save_plan(&self, plan: &Plan) -> SessionResult<()> {
        let c = &self.config;

        let status = serde_json::to_string(&plan.status)
            .unwrap_or_else(|_| "\"draft\"".to_string())
            .trim_matches('"')
            .to_string();

        sqlx::query(&format!(
            r#"
            INSERT INTO {plans} (
                id, session_id, name, content, status, error,
                created_at, approved_at, started_at, completed_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE SET
                name = EXCLUDED.name,
                content = EXCLUDED.content,
                status = EXCLUDED.status,
                error = EXCLUDED.error,
                approved_at = EXCLUDED.approved_at,
                started_at = EXCLUDED.started_at,
                completed_at = EXCLUDED.completed_at
            "#,
            plans = c.plans_table
        ))
        .bind(plan.id)
        .bind(plan.session_id.to_string())
        .bind(&plan.name)
        .bind(&plan.content)
        .bind(&status)
        .bind(&plan.error)
        .bind(plan.created_at)
        .bind(plan.approved_at)
        .bind(plan.started_at)
        .bind(plan.completed_at)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(())
    }

    async fn save_compacts(
        &self,
        session_id: &SessionId,
        compacts: &[CompactRecord],
    ) -> SessionResult<()> {
        let c = &self.config;

        // Delete existing and re-insert (simple approach for append-only history)
        sqlx::query(&format!(
            "DELETE FROM {compacts} WHERE session_id = $1",
            compacts = c.compacts_table
        ))
        .bind(session_id.to_string())
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        for compact in compacts {
            let trigger = serde_json::to_string(&compact.trigger)
                .unwrap_or_else(|_| "\"manual\"".to_string())
                .trim_matches('"')
                .to_string();

            sqlx::query(&format!(
                r#"
                INSERT INTO {compacts} (
                    id, session_id, trigger, pre_tokens, post_tokens, saved_tokens,
                    summary, original_count, new_count, logical_parent_id, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
                compacts = c.compacts_table
            ))
            .bind(compact.id)
            .bind(session_id.to_string())
            .bind(&trigger)
            .bind(compact.pre_tokens as i32)
            .bind(compact.post_tokens as i32)
            .bind(compact.saved_tokens as i32)
            .bind(&compact.summary)
            .bind(compact.original_count as i32)
            .bind(compact.new_count as i32)
            .bind(compact.logical_parent_id.as_ref().map(|id| id.to_string()))
            .bind(compact.created_at)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| SessionError::PersistenceError(e.to_string()))?;
        }

        Ok(())
    }

    async fn save_messages(
        &self,
        session_id: &SessionId,
        messages: &[SessionMessage],
    ) -> SessionResult<()> {
        let c = &self.config;

        // Delete existing messages and re-insert
        // This is used after compact when message list is replaced
        sqlx::query(&format!(
            "DELETE FROM {messages} WHERE session_id = $1",
            messages = c.messages_table
        ))
        .bind(session_id.to_string())
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        for message in messages {
            let role = serde_json::to_string(&message.role)
                .unwrap_or_else(|_| "\"user\"".to_string())
                .trim_matches('"')
                .to_string();

            sqlx::query(&format!(
                r#"
                INSERT INTO {messages} (
                    id, session_id, parent_id, role, content, is_sidechain,
                    is_compact_summary, model, request_id, usage, metadata,
                    environment, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                "#,
                messages = c.messages_table
            ))
            .bind(message.id.to_string())
            .bind(session_id.to_string())
            .bind(message.parent_id.as_ref().map(|id| id.to_string()))
            .bind(&role)
            .bind(serde_json::to_value(&message.content).unwrap_or_default())
            .bind(message.is_sidechain)
            .bind(message.is_compact_summary)
            .bind(&message.metadata.model)
            .bind(&message.metadata.request_id)
            .bind(
                message
                    .usage
                    .as_ref()
                    .and_then(|u| serde_json::to_value(u).ok()),
            )
            .bind(serde_json::to_value(&message.metadata).unwrap_or_default())
            .bind(
                message
                    .environment
                    .as_ref()
                    .and_then(|e| serde_json::to_value(e).ok()),
            )
            .bind(message.timestamp)
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| SessionError::PersistenceError(e.to_string()))?;
        }

        Ok(())
    }
}

#[async_trait]
impl Persistence for PostgresPersistence {
    fn name(&self) -> &str {
        "postgres"
    }

    async fn save(&self, session: &Session) -> SessionResult<()> {
        let c = &self.config;

        let session_type = serde_json::to_string(&session.session_type)
            .unwrap_or_else(|_| "\"main\"".to_string())
            .trim_matches('"')
            .to_string();

        let state = serde_json::to_string(&session.state)
            .unwrap_or_else(|_| "\"created\"".to_string())
            .trim_matches('"')
            .to_string();

        let mode = serde_json::to_string(&session.mode)
            .unwrap_or_else(|_| "\"default\"".to_string())
            .trim_matches('"')
            .to_string();

        sqlx::query(&format!(
            r#"
            INSERT INTO {sessions} (
                id, parent_id, tenant_id, session_type, state, mode,
                config, permission_policy, summary,
                total_input_tokens, total_output_tokens, total_cost_usd,
                current_leaf_id, static_context_hash, error,
                created_at, updated_at, expires_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            ON CONFLICT (id) DO UPDATE SET
                parent_id = EXCLUDED.parent_id,
                tenant_id = EXCLUDED.tenant_id,
                session_type = EXCLUDED.session_type,
                state = EXCLUDED.state,
                mode = EXCLUDED.mode,
                config = EXCLUDED.config,
                permission_policy = EXCLUDED.permission_policy,
                summary = EXCLUDED.summary,
                total_input_tokens = EXCLUDED.total_input_tokens,
                total_output_tokens = EXCLUDED.total_output_tokens,
                total_cost_usd = EXCLUDED.total_cost_usd,
                current_leaf_id = EXCLUDED.current_leaf_id,
                static_context_hash = EXCLUDED.static_context_hash,
                error = EXCLUDED.error,
                updated_at = EXCLUDED.updated_at,
                expires_at = EXCLUDED.expires_at
            "#,
            sessions = c.sessions_table
        ))
        .bind(session.id.to_string())
        .bind(session.parent_id.map(|id| id.to_string()))
        .bind(&session.tenant_id)
        .bind(&session_type)
        .bind(&state)
        .bind(&mode)
        .bind(serde_json::to_value(&session.config).unwrap_or_default())
        .bind(serde_json::to_value(&session.permission_policy).unwrap_or_default())
        .bind(&session.summary)
        .bind(session.total_usage.input_tokens as i64)
        .bind(session.total_usage.output_tokens as i64)
        .bind(session.total_cost_usd)
        .bind(session.current_leaf_id.as_ref().map(|id| id.to_string()))
        .bind(&session.static_context_hash)
        .bind(&session.error)
        .bind(session.created_at)
        .bind(session.updated_at)
        .bind(session.expires_at)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        // Save related data to separate tables
        // Always sync to ensure empty collections are reflected in DB
        self.save_messages(&session.id, &session.messages).await?;
        self.save_todos(&session.id, &session.todos).await?;
        self.save_compacts(&session.id, &session.compact_history)
            .await?;

        if let Some(ref plan) = session.current_plan {
            self.save_plan(plan).await?;
        }

        Ok(())
    }

    async fn load(&self, id: &SessionId) -> SessionResult<Option<Session>> {
        match self.load_session_row(id).await {
            Ok(session) => Ok(Some(session)),
            Err(SessionError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn delete(&self, id: &SessionId) -> SessionResult<bool> {
        let c = &self.config;

        let result = sqlx::query(&format!(
            "DELETE FROM {sessions} WHERE id = $1",
            sessions = c.sessions_table
        ))
        .bind(id.to_string())
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn list(&self, tenant_id: Option<&str>) -> SessionResult<Vec<SessionId>> {
        let c = &self.config;

        let rows = if let Some(tid) = tenant_id {
            sqlx::query(&format!(
                "SELECT id FROM {sessions} WHERE tenant_id = $1",
                sessions = c.sessions_table
            ))
            .bind(tid)
            .fetch_all(self.pool.as_ref())
            .await
        } else {
            sqlx::query(&format!(
                "SELECT id FROM {sessions}",
                sessions = c.sessions_table
            ))
            .fetch_all(self.pool.as_ref())
            .await
        }
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get::<&str, _>("id").ok()?.parse().ok())
            .collect())
    }

    async fn list_children(&self, parent_id: &SessionId) -> SessionResult<Vec<SessionId>> {
        let c = &self.config;

        let rows = sqlx::query(&format!(
            "SELECT id FROM {sessions} WHERE parent_id = $1",
            sessions = c.sessions_table
        ))
        .bind(parent_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get::<&str, _>("id").ok()?.parse().ok())
            .collect())
    }

    async fn add_summary(&self, snapshot: SummarySnapshot) -> SessionResult<()> {
        let c = &self.config;

        sqlx::query(&format!(
            r#"
            INSERT INTO {summaries} (id, session_id, summary, leaf_message_id, created_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            summaries = c.summaries_table
        ))
        .bind(snapshot.id)
        .bind(snapshot.session_id.to_string())
        .bind(&snapshot.summary)
        .bind(snapshot.leaf_message_id.map(|id| id.to_string()))
        .bind(snapshot.created_at)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        sqlx::query(&format!(
            "UPDATE {sessions} SET summary = $1, updated_at = NOW() WHERE id = $2",
            sessions = c.sessions_table
        ))
        .bind(&snapshot.summary)
        .bind(snapshot.session_id.to_string())
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(())
    }

    async fn get_summaries(&self, session_id: &SessionId) -> SessionResult<Vec<SummarySnapshot>> {
        let c = &self.config;

        let rows = sqlx::query(&format!(
            r#"
            SELECT id, session_id, summary, leaf_message_id, created_at
            FROM {summaries}
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
            summaries = c.summaries_table
        ))
        .bind(session_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                Some(SummarySnapshot {
                    id: row.try_get("id").ok()?,
                    session_id: row.try_get::<&str, _>("session_id").ok()?.parse().ok()?,
                    summary: row.try_get("summary").ok()?,
                    leaf_message_id: row
                        .try_get::<&str, _>("leaf_message_id")
                        .ok()
                        .and_then(|s| s.parse().ok()),
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                })
            })
            .collect())
    }

    async fn enqueue(
        &self,
        session_id: &SessionId,
        content: String,
        priority: i32,
    ) -> SessionResult<QueueItem> {
        let c = &self.config;
        let item = QueueItem::enqueue(*session_id, &content).with_priority(priority);

        sqlx::query(&format!(
            r#"
            INSERT INTO {queue} (id, session_id, operation, content, priority, status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            queue = c.queue_table
        ))
        .bind(item.id)
        .bind(session_id.to_string())
        .bind("enqueue")
        .bind(&content)
        .bind(priority)
        .bind("pending")
        .bind(item.created_at)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(item)
    }

    async fn dequeue(&self, session_id: &SessionId) -> SessionResult<Option<QueueItem>> {
        let c = &self.config;

        let row = sqlx::query(&format!(
            r#"
            UPDATE {queue}
            SET status = 'processing'
            WHERE id = (
                SELECT id FROM {queue}
                WHERE session_id = $1 AND status = 'pending'
                ORDER BY priority DESC, created_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, session_id, operation, content, priority, status, created_at, processed_at
            "#,
            queue = c.queue_table
        ))
        .bind(session_id.to_string())
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(row.and_then(|row| {
            Some(QueueItem {
                id: row.try_get("id").ok()?,
                session_id: row.try_get::<&str, _>("session_id").ok()?.parse().ok()?,
                operation: super::types::QueueOperation::Enqueue,
                content: row.try_get("content").ok()?,
                priority: row.try_get("priority").unwrap_or(0),
                status: QueueStatus::Processing,
                created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                processed_at: row.try_get("processed_at").ok(),
            })
        }))
    }

    async fn cancel_queued(&self, item_id: Uuid) -> SessionResult<bool> {
        let c = &self.config;

        let result = sqlx::query(&format!(
            "UPDATE {queue} SET status = 'cancelled', processed_at = NOW() WHERE id = $1 AND status = 'pending'",
            queue = c.queue_table
        ))
        .bind(item_id)
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn pending_queue(&self, session_id: &SessionId) -> SessionResult<Vec<QueueItem>> {
        let c = &self.config;

        let rows = sqlx::query(&format!(
            r#"
            SELECT id, session_id, operation, content, priority, status, created_at, processed_at
            FROM {queue}
            WHERE session_id = $1 AND status = 'pending'
            ORDER BY priority DESC, created_at ASC
            "#,
            queue = c.queue_table
        ))
        .bind(session_id.to_string())
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                Some(QueueItem {
                    id: row.try_get("id").ok()?,
                    session_id: row.try_get::<&str, _>("session_id").ok()?.parse().ok()?,
                    operation: super::types::QueueOperation::Enqueue,
                    content: row.try_get("content").ok()?,
                    priority: row.try_get("priority").unwrap_or(0),
                    status: QueueStatus::Pending,
                    created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
                    processed_at: row.try_get("processed_at").ok(),
                })
            })
            .collect())
    }

    async fn cleanup_expired(&self) -> SessionResult<usize> {
        let c = &self.config;

        let result = sqlx::query(&format!(
            "DELETE FROM {sessions} WHERE expires_at IS NOT NULL AND expires_at < NOW()",
            sessions = c.sessions_table
        ))
        .execute(self.pool.as_ref())
        .await
        .map_err(|e| SessionError::PersistenceError(e.to_string()))?;

        Ok(result.rows_affected() as usize)
    }
}
