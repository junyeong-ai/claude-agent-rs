# Session Management

Stateful conversation management with multi-backend persistence, prompt caching, and context compaction.

> **Related**: [Memory System Guide](memory-system.md) for CLAUDE.md loading and @import

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
│                   └── Persistence (Memory / JSONL / PostgreSQL / Redis)  │
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
    pub compact_history: VecDeque<CompactRecord>,
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
    .threshold(0.85)
    .model("claude-haiku-4-5");

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
    async fn cancel_queued(&self, item_id: Uuid) -> SessionResult<bool>;
    async fn pending_queue(&self, session_id: &SessionId) -> SessionResult<Vec<QueueItem>>;

    // Cleanup
    async fn cleanup_expired(&self) -> SessionResult<usize>;
}
```

### Backends

| Backend | Feature | Use Case |
|---------|---------|----------|
| `MemoryPersistence` | (default) | Development |
| `JsonlPersistence` | `jsonl` | CLI-compatible (`~/.claude/projects/`) |
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
let persistence = PostgresPersistence::connect("postgres://localhost/mydb").await?;
// Or with auto-migration:
// let persistence = PostgresPersistence::connect_and_migrate("postgres://localhost/mydb").await?;

// From existing pool with custom prefix:
let config = PostgresConfig::prefix("myapp_")?;
let persistence = PostgresPersistence::pool_and_config(pool, config);
```

### Redis

```rust
let persistence = RedisPersistence::new("redis://localhost:6379")?
    .prefix("myapp:")?
    .ttl(Duration::from_secs(86400 * 7));
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
│   [System: 1h TTL] [User₁] ... [User_n: 5m TTL]                         │
│         ↑                           ↑                                    │
│   Static content              Last user turn                             │
│   (long cache)                (short cache)                              │
│                                                                          │
│   Turn 1: [System] [User₁]                    → cache_creation           │
│   Turn 2: [System] [User₁] [Asst₁] [User₂]   → cache_read + creation    │
│   Turn 3: [System] ... [User₃]                → cache_read               │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Configuration

```rust
use claude_agent::{CacheConfig, CacheStrategy, CacheTtl};

// Default: Full caching (system: 1h, messages: 5m)
let config = CacheConfig::default();

// System prompt only (1h TTL)
let config = CacheConfig::system_only();

// Messages only (5m TTL)
let config = CacheConfig::messages_only();

// Disabled
let config = CacheConfig::disabled();

// Custom TTL
let config = CacheConfig::default()
    .static_ttl(CacheTtl::OneHour)
    .message_ttl(CacheTtl::FiveMinutes);

// Custom strategy
let config = CacheConfig::default()
    .strategy(CacheStrategy::SystemOnly);

Agent::builder()
    .auth(Auth::from_env()).await?
    .cache(config)
    .build()
    .await?
```

### Cache Strategy

| Strategy | System Prompt | Messages | Use Case |
|----------|---------------|----------|----------|
| `Full` (default) | 1h TTL | 5m TTL | Multi-turn conversations |
| `SystemOnly` | 1h TTL | No | Short conversations |
| `MessagesOnly` | No | 5m TTL | Dynamic system prompts |
| `Disabled` | No | No | Testing / debugging |

### How It Works

1. **System prompt caching**: Static context (CLAUDE.md, tools, skills) cached with 1-hour TTL
2. **Message history caching**: Last user message marked with `cache_control: ephemeral` and 5-minute TTL
3. **TTL ordering**: Long TTL content must come before short TTL content (Anthropic requirement)

### Cost Impact

| Token Type | Cost Multiplier | TTL |
|------------|-----------------|-----|
| `cache_creation` (5m) | 1.25x input cost | 5 minutes |
| `cache_creation` (1h) | 2.0x input cost | 1 hour |
| `cache_read` | 0.1x input cost | - |
| Regular input | 1.0x | - |

### Metrics

```rust
// AgentMetrics cache fields
pub struct AgentMetrics {
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    // ...
}

// Cache metrics
let hit_rate = metrics.cache_hit_rate();      // cache_read / input
let efficiency = metrics.cache_efficiency();  // reads / (reads + writes)
let saved = metrics.cache_tokens_saved();     // cache_read * 0.9

// Cost savings calculation
let savings = metrics.cache_cost_savings(3.0);  // $3/MTok
```

## Error Handling

```rust
pub enum SessionError {
    NotFound { id: String },
    Expired { id: String },
    Storage { message: String },
    Serialization(serde_json::Error),
    Compact { message: String },
    Context(ContextError),
}
```
