# Session Management & Prompt Caching

Session management for stateful conversations with Prompt Caching support.

## Architecture

Two distinct layers handle conversation state:

| Layer | Component | Scope | Persistence |
|-------|-----------|-------|-------------|
| **Execution** | `ConversationContext` | Single `execute()` call | Ephemeral |
| **Persistence** | `Session` | Multi-turn, resumable | Via `Persistence` trait |

```
Agent.execute(prompt)
    │
    ├──▶ ConversationContext  ← Active execution buffer
    │        ├── messages: Vec<Message>
    │        ├── usage tracking
    │        └── auto-compact (80% threshold)
    │
    └──▶ session_id (string)  ← Reference only
              │
              ▼
         SessionManager  ← Optional persistence layer
              │
              ▼
         Session (tree structure, multi-tenant)
```

**Design rationale**: Separation enables lightweight execution without persistence overhead.
Use `AgentBuilder::resume_session()` for multi-turn conversations.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                  Session & Caching Flow                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐   │
│   │   Static    │────▶│   Cache     │────▶│   API       │   │
│   │   Context   │     │   Manager   │     │   Request   │   │
│   └─────────────┘     └─────────────┘     └─────────────┘   │
│         │                   │                   │            │
│         │                   │                   │            │
│         ▼                   ▼                   ▼            │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐   │
│   │ cache_control│    │  CacheStats │     │  Response   │   │
│   │ : ephemeral │     │  tracking   │     │  + usage    │   │
│   └─────────────┘     └─────────────┘     └─────────────┘   │
│                                                              │
│   ┌──────────────────────────────────────────────────────┐  │
│   │              CompactExecutor (80% threshold)          │  │
│   │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  │  │
│   │  │ Detect  │─▶│Summarize│─▶│ Replace │─▶│  Keep   │  │  │
│   │  │threshold│  │ older   │  │ messages│  │ recent  │  │  │
│   │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  │  │
│   └──────────────────────────────────────────────────────┘  │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Prompt Caching

Anthropic's Prompt Caching reduces costs by caching static content.

### How It Works

1. Static content marked with `cache_control: { type: "ephemeral" }`
2. First request creates cache (cache_creation_input_tokens)
3. Subsequent requests read from cache (cache_read_input_tokens)
4. Cache reads are ~90% cheaper than creation

### CacheStats

```rust
pub struct CacheStats {
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64;      // cache_hits / total
    pub fn tokens_saved(&self) -> u64;  // ~90% of cache_read_tokens
}
```

### SessionCacheManager

```rust
let mut cache_manager = SessionCacheManager::new();

// Initialize with static context
cache_manager.initialize(&static_context);

// Build system blocks with cache_control
let blocks = cache_manager.build_cached_system(&static_context);

// Check for context changes
if cache_manager.has_context_changed(&new_context) {
    cache_manager.update_context(&new_context);
}

// Record API response usage
cache_manager.record_usage(cache_read, cache_creation);

// Get statistics
let stats = cache_manager.stats();
println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
```

### Disabling Caching

```rust
let cache_manager = SessionCacheManager::disabled();
// or
let mut cache_manager = SessionCacheManager::new();
cache_manager.disable();
```

### CacheConfigBuilder

```rust
let cache_manager = CacheConfigBuilder::new()
    .with_breakpoint("system", 0)   // Priority 0 = earliest
    .with_breakpoint("context", 10)
    .build();

// Or disabled
let cache_manager = CacheConfigBuilder::new()
    .disabled()
    .build();
```

## Session State

### Session

```rust
pub struct Session {
    pub id: SessionId,
    pub tenant_id: Option<String>,
    pub mode: SessionMode,
    pub state: SessionState,
    pub config: SessionConfig,
    pub permission_policy: PermissionPolicy,
    pub messages: Vec<SessionMessage>,
    pub current_leaf_id: Option<MessageId>,
    pub summary: Option<String>,           // After compact
    pub total_usage: TokenUsage,
    pub total_cost_usd: f64,
    pub static_context_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}
```

### SessionConfig

```rust
pub struct SessionConfig {
    pub model: String,                     // Default: "claude-sonnet-4-5"
    pub max_tokens: u32,                   // Default: 16384
    pub permission_policy: PermissionPolicy,
    pub mode: SessionMode,
    pub ttl_secs: Option<u64>,             // None = no expiry
    pub system_prompt: Option<String>,
}
```

### SessionMode

```rust
pub enum SessionMode {
    Stateless,                            // Single request
    Stateful { persistence: String },     // Persistent
}
```

### SessionState

```rust
pub enum SessionState {
    Created,          // Newly created
    Active,           // Running
    WaitingForTools,  // Waiting for tool results
    WaitingForUser,   // Waiting for user input
    Paused,           // Paused
    Completed,        // Finished
    Error,            // Error state
}
```

### SessionMessage (Tree Node)

```rust
pub struct SessionMessage {
    pub id: MessageId,
    pub parent_id: Option<MessageId>,     // Tree structure
    pub role: Role,
    pub content: Vec<ContentBlock>,
    pub is_sidechain: bool,               // Branch flag
    pub usage: Option<TokenUsage>,
    pub timestamp: DateTime<Utc>,
    pub metadata: MessageMetadata,
}

// Factory methods
SessionMessage::user(content)
SessionMessage::assistant(content)

// Builder methods
.with_parent(parent_id)
.with_usage(usage)
.as_sidechain()
```

### Message Tree Navigation

```rust
let mut session = Session::new(SessionConfig::default());

// Add messages (automatically links parent)
session.add_message(SessionMessage::user(vec![ContentBlock::text("Hello")]));
session.add_message(SessionMessage::assistant(vec![ContentBlock::text("Hi!")]));

// Get current branch (root to leaf)
let branch = session.get_current_branch();

// Convert to API messages
let api_messages = session.to_api_messages();

// Branch length
let len = session.branch_length();
```

## Context Compaction

When context exceeds threshold, older messages are summarized.

### CompactStrategy

```rust
pub struct CompactStrategy {
    pub enabled: bool,
    pub threshold_percent: f32,     // Default: 0.8 (80%)
    pub summary_model: String,      // Default: claude-haiku
    pub keep_recent_messages: usize, // Default: 4
    pub max_summary_tokens: u32,    // Default: 2000
}

// Builders
CompactStrategy::default()
CompactStrategy::disabled()
    .with_threshold(0.85)
    .with_model("claude-haiku-4-5")
    .with_keep_recent(6)
```

### CompactExecutor

```rust
let executor = CompactExecutor::new(CompactStrategy::default());

// Check if compact needed
if executor.needs_compact(current_tokens, max_tokens) {
    // Prepare compact
    let prepared = executor.prepare_compact(&session)?;

    match prepared {
        PreparedCompact::NotNeeded => { /* Do nothing */ }
        PreparedCompact::Ready { summary_prompt, messages_to_keep, .. } => {
            // Generate summary via API call
            let summary = call_api_for_summary(summary_prompt).await?;

            // Apply compact
            let result = executor.apply_compact(&mut session, summary, messages_to_keep);

            match result {
                CompactResult::Compacted { original_count, new_count, saved_tokens, .. } => {
                    println!("Compacted: {} -> {} messages", original_count, new_count);
                }
                _ => {}
            }
        }
    }
}
```

### Compact Flow

1. **Check threshold**: `needs_compact(current_tokens, max_tokens)`
2. **Prepare**: Split messages into to-summarize and to-keep
3. **Generate summary**: Call API with summary prompt (external)
4. **Apply**: Replace old messages with summary + recent messages
5. **Update state**: Set `session.summary`, update `current_leaf_id`

## Persistence

### Persistence Trait

```rust
pub trait Persistence: Send + Sync {
    async fn save(&self, session: &Session) -> SessionResult<()>;
    async fn load(&self, id: &SessionId) -> SessionResult<Option<Session>>;
    async fn delete(&self, id: &SessionId) -> SessionResult<()>;
    async fn list(&self) -> SessionResult<Vec<SessionId>>;
}
```

### MemoryPersistence

In-memory storage (for testing/development):

```rust
let persistence = MemoryPersistence::new();

persistence.save(&session).await?;
let loaded = persistence.load(&session.id).await?;
persistence.delete(&session.id).await?;
```

## Session Manager

High-level session operations.

```rust
let manager = SessionManager::new(persistence);

// Create session
let session = manager.create(SessionConfig::default()).await?;

// Get session
let session = manager.get(&session_id).await?;

// Update session
manager.update(&session).await?;

// Delete session
manager.delete(&session_id).await?;

// List sessions
let sessions = manager.list().await?;
```

## Token Usage Tracking

```rust
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

// Accumulate across messages
session.total_usage.add(&message_usage);
```

## Error Types

```rust
pub enum SessionError {
    NotFound { id: String },
    Expired { id: String },
    PermissionDenied { reason: String },
    Storage { message: String },
    Serialization(serde_json::Error),
    Compact { message: String },
    Context(ContextError),
}
```

## File Locations

| Type | File | Line |
|------|------|------|
| `SessionError` | session/mod.rs | 23-67 |
| `CacheStats` | session/cache.rs | 16-55 |
| `SessionCacheManager` | session/cache.rs | 57-184 |
| `CacheConfigBuilder` | session/cache.rs | 192-245 |
| `CompactStrategy` | session/compact.rs | 10-53 |
| `CompactExecutor` | session/compact.rs | 55-195 |
| `PreparedCompact` | session/compact.rs | 197-211 |
| `SessionId` | session/state.rs | 11-41 |
| `MessageId` | session/state.rs | 43-63 |
| `SessionMode` | session/state.rs | 65-78 |
| `SessionState` | session/state.rs | 80-99 |
| `PermissionMode` | session/state.rs | 101-114 |
| `PermissionPolicy` | session/state.rs | 116-130 |
| `SessionConfig` | session/state.rs | 141-158 |
| `SessionMessage` | session/state.rs | 208-285 |
| `Session` | session/state.rs | 287-413 |
| `SessionManager` | session/manager.rs | - |
| `Persistence` trait | session/persistence.rs | - |
| `MemoryPersistence` | session/persistence.rs | - |
