# claude-agent-rs

Rust SDK for Claude API. 449 tests, 0 warnings.

## Architecture

```
src/
├── agent/          # Agent loop, executor, state machine
├── auth/           # AuthStrategy: ApiKey, OAuth, Bedrock, Vertex, Foundry
├── client/         # HTTP client, streaming, messages
├── tools/          # Tool trait, 11 built-in tools
├── skills/         # SkillDefinition, YAML frontmatter, triggers
├── context/        # MemoryLoader (CLAUDE.md, @import), orchestrator
├── config/         # Settings, permissions
├── session/        # Persistence, compaction
├── hooks/          # Pre/post execution hooks
├── mcp/            # MCP server integration (stdio, http, sse)
└── permissions/    # PermissionMode, rules
```

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `Agent` | agent/mod.rs | Main entry, builder pattern |
| `Client` | client/mod.rs | API client, streaming |
| `AuthStrategy` | auth/strategy/traits.rs | Auth abstraction |
| `Tool` | tools/mod.rs:47 | Tool trait |
| `SkillDefinition` | skills/mod.rs | Skill with frontmatter |
| `MemoryLoader` | context/memory_loader.rs | CLAUDE.md loader |

## Patterns

**AuthStrategy**: `auth_header()`, `extra_headers()`, `prepare_request()`
**Tool**: `name()`, `description()`, `input_schema()`, `execute()`
**Builder**: `Client::builder()`, `Agent::builder()`

## Modification Points

| Task | Files |
|------|-------|
| Add cloud provider | `auth/strategy/` + `client/config.rs` |
| Add built-in tool | `tools/` + `tools/mod.rs` (register) |
| Modify agent loop | `agent/executor.rs` |
| Add skill feature | `skills/mod.rs`, `skills/loader.rs` |

## Commands

```bash
cargo test
cargo clippy --all-features
cargo doc --open
```

## Exports (lib.rs)

```rust
pub use agent::{Agent, AgentBuilder, AgentEvent, AgentResult};
pub use auth::{AuthStrategy, BedrockStrategy, VertexStrategy, FoundryStrategy};
pub use client::{Client, ClientBuilder, CloudProvider};
pub use tools::{Tool, ToolAccess, ToolRegistry, ToolResult};
pub use skills::{SkillDefinition, SkillExecutor};
```
