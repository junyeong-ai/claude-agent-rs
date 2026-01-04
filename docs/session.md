# Session Management

Stateful conversation management with multi-backend persistence, prompt caching, and context compaction.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          Session Architecture                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Agent.execute(prompt)                                                  │
│         │                                                                │
│         ├──▶ ConversationContext    ← Active execution                  │
│         │         └── auto-compact (80% threshold)                       │
│         │                                                                │
│         └──▶ SessionManager         ← Persistence layer                 │
│                   ├── Session (messages, todos, plans, compacts)         │
│                   ├── InputQueue (concurrent inputs)                     │
│                   └── Persistence (Memory / PostgreSQL / Redis)          │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Session

All session data in a single struct:

```rust
pub struct Session {
    pub id: SessionId,
    pub parent_id: Option<SessionId>,
    pub session_type: SessionType,
    pub tenant_id: Option<String>,
    pub messages: Vec<SessionMessage>,
    pub todos: Vec<TodoItem>,
    pub current_plan: Option<Plan>,
    pub compact_history: Vec<CompactRecord>,
    pub summary: Option<String>,
    pub total_usage: TokenUsage,
    // ...
}
```

## Context Compaction

Claude Code compatible: summarizes **entire conversation**.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       Compaction (80% threshold)                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Before:  [M1] [M2] [M3] [M4] [M5] [M6] [M7] [M8]  (100k tokens)       │
│                          ↓                                               │
│                    /compact or auto                                      │
│                          ↓                                               │
│   After:   [Summary of entire conversation]        (5k tokens)           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

```rust
let strategy = CompactStrategy::default()
    .with_threshold(0.85)
    .with_model("claude-haiku-4-5");

let executor = CompactExecutor::new(strategy);

if executor.needs_compact(current_tokens, max_tokens) {
    let prepared = executor.prepare_compact(&session)?;
    if let PreparedCompact::Ready { summary_prompt, .. } = prepared {
        let summary = generate_summary(summary_prompt).await?;
        let result = executor.apply_compact(&mut session, summary);
        executor.record_compact(&mut session, &result);
    }
}
```

## Persistence

### Trait Interface

```rust
#[async_trait]
pub trait Persistence: Send + Sync {
    fn name(&self) -> &str;

    // Session CRUD
    async fn save(&self, session: &Session) -> SessionResult<()>;
    async fn load(&self, id: &SessionId) -> SessionResult<Option<Session>>;
    async fn delete(&self, id: &SessionId) -> SessionResult<bool>;
    async fn list(&self, tenant_id: Option<&str>) -> SessionResult<Vec<SessionId>>;

    // Messages
    async fn add_message(&self, session_id: &SessionId, message: SessionMessage) -> SessionResult<()>;

    // Summaries
    async fn add_summary(&self, snapshot: SummarySnapshot) -> SessionResult<()>;
    async fn get_summaries(&self, session_id: &SessionId) -> SessionResult<Vec<SummarySnapshot>>;

    // Queue
    async fn enqueue(&self, session_id: &SessionId, content: String, priority: i32) -> SessionResult<QueueItem>;
    async fn dequeue(&self, session_id: &SessionId) -> SessionResult<Option<QueueItem>>;

    // Cleanup
    async fn cleanup_expired(&self) -> SessionResult<usize>;
}
```

### Backends

| Backend | Feature | Use Case |
|---------|---------|----------|
| `MemoryPersistence` | (default) | Development |
| `PostgresPersistence` | `postgres` | Production |
| `RedisPersistence` | `redis-backend` | High-throughput |

### PostgreSQL (7 tables)

```
claude_sessions ─┬─> claude_messages
                 ├─> claude_compacts
                 ├─> claude_summaries
                 ├─> claude_queue
                 ├─> claude_todos
                 └─> claude_plans
```

```rust
let persistence = PostgresPersistence::new("postgres://localhost/mydb").await?;
let persistence = PostgresPersistence::with_prefix(pool, "myapp_").await?;
```

### Redis

```rust
let persistence = RedisPersistence::new("redis://localhost:6379")?
    .with_prefix("myapp:")
    .with_ttl(Duration::from_secs(86400 * 7));
```

## Input Queue

Thread-safe queue for concurrent inputs:

```rust
let queue = SharedInputQueue::new();

let id = queue.enqueue(QueuedInput::new(session_id, "Hello")).await;

// Merge all (newline-joined, last environment)
if let Some(merged) = queue.merge_all().await {
    // merged.content = "Hello\nWorld"
}

queue.cancel(id).await;
```

## Prompt Caching

Automatic caching based on Anthropic best practices for cost reduction in multi-turn conversations.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       Prompt Caching Strategy                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Turn 1: [System] [User₁]                    → cache_creation           │
│   Turn 2: [System] [User₁] [Asst₁] [User₂]   → cache_read + creation    │
│   Turn 3: [System] [User₁] [Asst₁] [User₂] [Asst₂] [User₃] → cache_read │
│                                        ↑                                 │
│                              cache_control on last user                  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Configuration

```rust
use claude_agent::CacheConfig;

// Default: both enabled
let config = CacheConfig::default();

// System prompt only
let config = CacheConfig::system_only();

// Disabled
let config = CacheConfig::disabled();

// Custom
let config = CacheConfig {
    enabled: true,
    system_prompt_cache: true,
    message_cache: true,
};

Agent::builder()
    .cache(config)
    .build()
    .await?
```

### How It Works

1. **System prompt caching**: Static context (CLAUDE.md, tools, skills) cached automatically
2. **Message history caching**: Last user message marked with `cache_control: ephemeral`
3. **Cache TTL**: 5 minutes by default (Anthropic API)

### Cost Impact

| Token Type | Cost Multiplier |
|------------|-----------------|
| `cache_creation` | 1.25x input cost |
| `cache_read` | 0.1x input cost |
| Regular input | 1.0x |

### Tracking

```rust
// TokenUsage includes cache fields
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

// Cache hit rate
let hit_rate = usage.cache_hit_rate();  // cache_read / input
```

## Error Handling

```rust
pub enum SessionError {
    NotFound { id: String },
    Expired { id: String },
    PermissionDenied { reason: String },
    Storage { message: String },
    PersistenceError(String),
    Serialization(serde_json::Error),
    Compact { message: String },
    Plan { message: String },
}
```
