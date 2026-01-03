# claude-agent-rs

Rust SDK for Claude API. 1061 tests, 44k lines, 198 files.

## Architecture

```
src/
├── agent/          # Agent, AgentBuilder, TaskRegistry
├── auth/           # Auth enum (ApiKey, ClaudeCli, Bedrock, Vertex, Foundry)
├── client/         # Client, streaming, adapter/, resilience/
├── tools/          # 12 built-in tools, Tool/SchemaTool traits
├── skills/         # SkillDefinition, SkillExecutor, triggers
├── subagents/      # SubagentDefinition, 3 builtins
├── context/        # PromptOrchestrator, MemoryLoader, @import
├── session/        # Session, Persistence (Memory/Postgres/Redis)
├── security/       # SecureFs, BashAnalyzer, ResourceLimits, Sandbox
├── permissions/    # PermissionMode, PermissionPolicy, ToolLimits
├── hooks/          # Hook trait, HookManager, 10 events
├── mcp/            # McpClient, McpManager (stdio/sse)
├── budget/         # BudgetTracker, TenantBudgetManager
├── observability/  # Metrics, Tracing, OpenTelemetry
├── output_style/   # OutputStyle, SystemPromptGenerator
└── types/          # ContentBlock, Message, ServerTool (WebFetch, WebSearch)
```

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `Agent` | agent/executor.rs | `execute()`, `execute_stream()` |
| `AgentBuilder` | agent/builder.rs | Fluent construction |
| `Tool` | tools/traits.rs | `name()`, `description()`, `input_schema()`, `execute()` |
| `SchemaTool` | tools/traits.rs | Auto schema via schemars |
| `Auth` | auth/mod.rs | ApiKey, ClaudeCli, Bedrock, Vertex, Foundry |
| `ProviderAdapter` | client/adapter/traits.rs | Multi-cloud abstraction |
| `SecurityContext` | security/mod.rs | fs + bash + limits + network + sandbox |
| `Persistence` | session/persistence.rs | Session storage trait |
| `HookManager` | hooks/manager.rs | Priority-based hook execution |
| `McpManager` | mcp/manager.rs | `mcp__server__tool` naming |

## 12 Built-in Tools

```
Read, Write, Edit, Glob, Grep, Bash, KillShell,
Task, TaskOutput, TodoWrite, Plan, Skill
```

**Server tools** (Anthropic direct API only): `WebFetch`, `WebSearch`

## 3 Built-in Subagents

| Name | Model | Tools | Purpose |
|------|-------|-------|---------|
| `explore` | Small (Haiku) | Read, Grep, Glob, Bash | Fast codebase search |
| `plan` | Primary | All | Implementation planning |
| `general` | Primary | All | Complex multi-step tasks |

## 10 Hook Events

| Event | Blockable | Event | Blockable |
|-------|-----------|-------|-----------|
| PreToolUse | Yes | SubagentStart | No |
| PostToolUse | No | SubagentStop | No |
| PostToolUseFailure | No | PreCompact | No |
| UserPromptSubmit | Yes | SessionStart | No |
| Stop | No | SessionEnd | No |

## Persistence Backends

| Backend | Feature | Tables |
|---------|---------|--------|
| `MemoryPersistence` | (default) | - |
| `PostgresPersistence` | `postgres` | 7 tables (claude_*) |
| `RedisPersistence` | `redis-backend` | - |

## Auth Methods (8 variants)

| Method | Feature | Usage |
|--------|---------|-------|
| `ApiKey` | - | `Auth::api_key("sk-...")` |
| `FromEnv` | - | `Auth::from_env()` (ANTHROPIC_API_KEY) |
| `ClaudeCli` | cli-integration | `Auth::claude_cli()` |
| `OAuth` | - | `Auth::oauth("token")` |
| `Resolved` | - | `Auth::resolved(credential)` |
| `Bedrock` | aws | `Auth::bedrock("us-east-1")` |
| `Vertex` | gcp | `Auth::vertex("project", "region")` |
| `Foundry` | azure | `Auth::foundry("resource")` |

## Modification Points

| Task | Location |
|------|----------|
| Add tool | impl `SchemaTool` + `tools/builder.rs` |
| Add auth | `auth/providers/` |
| Add cloud adapter | impl `ProviderAdapter` in `client/adapter/` |
| Add skill | `.claude/skills/*.md` or `SkillDefinition::new()` |
| Add subagent | `.claude/agents/*.md` or `SubagentDefinition::new()` |
| Add hook | impl `Hook` + `HookManager::register()` |
| Add MCP server | `McpManager::add_server()` |
| Modify security | `security/` (fs/, bash/, limits/, sandbox/) |

## Patterns

```rust
// SchemaTool: auto JSON schema
impl SchemaTool for MyTool {
    type Input = MyInput;  // #[derive(JsonSchema, Deserialize)]
    const NAME: &'static str = "my_tool";
    const DESCRIPTION: &'static str = "...";
    async fn handle(&self, input: Self::Input, ctx: &ExecutionContext) -> ToolResult;
}

// Agent builder
Agent::builder()
    .from_claude_code()         // CLI OAuth
    .tools(ToolAccess::all())
    .with_web_search()
    .build()
    .await?;
```

## Security

- **TOCTOU-safe**: `openat()` + `O_NOFOLLOW`, symlink depth=10
- **Bash AST**: tree-sitter, DangerLevel detection
- **Limits**: `setrlimit()` (CPU, memory, files, processes)
- **Sandbox**: Landlock (Linux 5.13+), Seatbelt (macOS)
- **Network**: Domain whitelist/blacklist

## Features

```toml
default = ["cli-integration"]
mcp = ["rmcp"]
multimedia = ["pdf-extract"]
aws, gcp, azure           # Cloud providers
cloud-all = ["aws", "gcp", "azure"]
postgres, redis-backend   # Persistence
persistence-all = ["postgres", "redis-backend"]
otel                      # OpenTelemetry
full = ["mcp", "cloud-all", "persistence-all", "otel"]
```

## Commands

```bash
cargo test                    # 1061 tests
cargo test -- --ignored       # + live API tests
cargo clippy --all-features
```
