# claude-agent-rs

Rust SDK for Claude API. 516 tests.

## Architecture

```
src/
├── agent/          # Agent, AgentBuilder, executor, state
├── auth/           # AuthStrategy: ApiKey, OAuth, Bedrock, Vertex, Foundry
├── client/         # Client, streaming, messages
├── tools/          # Tool trait, 13 built-in, ProcessManager
├── skills/         # SkillDefinition, triggers, CommandLoader
├── context/        # MemoryLoader, ContextOrchestrator, StaticContext
├── session/        # SessionCacheManager, CacheStats, compaction
├── extension/      # Extension trait (Bevy-style lifecycle)
├── hooks/          # Pre/post execution hooks
├── mcp/            # MCP server (stdio, http, sse)
└── permissions/    # PermissionMode, rules
```

## Key Types

| Type | File:Line | Purpose |
|------|-----------|---------|
| `Agent` | agent/mod.rs:50 | Builder pattern entry |
| `Tool` | tools/registry.rs:52 | `name()`, `description()`, `input_schema()`, `execute()` |
| `TypedTool` | tools/registry.rs:87 | Internal: auto schema from `schemars` |
| `AuthStrategy` | auth/strategy/traits.rs:15 | `auth_header()`, `prepare_request()` |
| `SkillDefinition` | skills/mod.rs:45 | `with_trigger()`, `$ARGUMENTS` |
| `ProcessManager` | tools/process.rs:20 | Shared bash/kill state |
| `CacheStats` | session/cache.rs:32 | `hit_rate()`, `tokens_saved()` |

## 13 Built-in Tools

`ToolRegistry::default_tools()` in `tools/registry.rs:145`:
```
Read, Write, Edit, Glob, Grep, NotebookEdit,
Bash, KillShell (shared ProcessManager),
WebFetch, Task, TaskOutput, TodoWrite, Skill
```

## Modification Points

| Task | Files |
|------|-------|
| Add tool | `tools/*.rs` + `tools/registry.rs:152` (register) |
| Add auth provider | `auth/strategy/` + `auth/providers/` |
| Modify executor | `agent/executor.rs` |
| Add skill feature | `skills/mod.rs`, `skills/loader.rs` |
| Add extension | impl `Extension` trait |

## Key Patterns

```rust
// Tool: TypedTool auto-implements Tool via blanket impl
#[async_trait]
impl TypedTool for MyTool {
    type Input = MyInput;  // derives JsonSchema
    const NAME: &'static str = "MyTool";
    async fn handle(&self, input: Self::Input) -> ToolResult;
}

// Auth: prepare_request modifies request before send
fn prepare_request(&self, req: CreateMessageRequest) -> CreateMessageRequest;

// Builder: async for Agent, sync for Client
Agent::builder().from_claude_cli().build().await?;
Client::builder().from_claude_cli().build()?;
```

## CLI Auth Flow

`from_claude_cli()` → `ClaudeCliProvider` → `~/.claude/credentials.json` → `OAuthStrategy`

OAuth adds: `Authorization: Bearer`, `anthropic-beta`, `cache_control: ephemeral`

## Commands

```bash
cargo test                    # unit tests
cargo test -- --ignored       # + live API tests
cargo clippy --all-features
```
