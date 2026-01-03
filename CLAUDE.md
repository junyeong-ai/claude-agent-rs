# claude-agent-rs

Rust SDK for Claude API. ~1,000 tests, ~42k lines.

## Architecture

```
src/
├── agent/          # Agent, executor, TaskRegistry
├── auth/           # CredentialProvider: OAuth, ApiKey, Bedrock, Vertex, Foundry
├── client/         # Client, streaming, adapter/ (multi-cloud)
├── tools/          # 12 built-in, Tool trait, ToolRegistry
├── skills/         # SkillDefinition, CommandLoader, triggers
├── subagents/      # SubagentDefinition, 3 builtins (explore, plan, general)
├── context/        # MemoryLoader, RuleIndex, @import
├── session/        # ToolState, persistence, compaction
├── security/       # TOCTOU-safe fs, BashAnalyzer, ResourceLimits, Sandbox
├── permissions/    # PermissionMode, rules, ToolLimits
├── output_style/   # OutputStyle, SystemPromptGenerator
├── mcp/            # McpClient, McpManager (stdio, http, sse)
├── hooks/          # Pre/post execution hooks
├── budget/         # BudgetTracker, TenantBudgetManager
└── types/          # ServerTool (WebFetch, WebSearch), ContentBlock
```

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `Agent` | agent/executor.rs | `execute()` / `execute_stream()` |
| `Tool` | tools/traits.rs:12 | `name()`, `description()`, `input_schema()`, `execute()` |
| `SchemaTool` | tools/traits.rs:29 | Auto schema via schemars (blanket impl) |
| `ToolRegistry` | tools/registry.rs:20 | 12 built-in, dynamic registration |
| `ToolState` | session/session_state.rs:72 | In-memory state (todos, plans) |
| `CredentialProvider` | auth/provider.rs | `resolve()`, `refresh()` |
| `ProviderAdapter` | client/adapter/traits.rs | Multi-cloud abstraction |
| `SkillDefinition` | skills/mod.rs:34 | `with_trigger()`, `with_allowed_tools()` |
| `SubagentDefinition` | subagents/mod.rs:32 | Independent context agents |
| `SecurityContext` | security/mod.rs:35 | fs + bash + limits + network |
| `HookManager` | hooks/manager.rs:40 | Priority-based execution |
| `McpManager` | mcp/manager.rs:22 | Multi-server, `mcp__server__tool` naming |
| `SessionCacheManager` | session/cache.rs:58 | Prompt caching, cache_control |
| `CompactExecutor` | session/compact.rs:55 | Context compaction (80% threshold) |
| `BudgetTracker` | budget/tracker.rs | Token/cost limits per tenant |

## 12 Built-in Tools

```
Read, Write, Edit, Glob, Grep, Bash, KillShell,
Task, TaskOutput, TodoWrite, Plan, Skill
```

Registered in `tools/builder.rs:132-145`.

**Server tools** (Anthropic API): `WebFetch`, `WebSearch` in `types/tool/server.rs`.

## Modification Points

| Task | Files |
|------|-------|
| Add tool | impl `SchemaTool` + register in `tools/builder.rs:132` |
| Add auth | impl `CredentialProvider` in `auth/providers/` |
| Add cloud adapter | impl `ProviderAdapter` in `client/adapter/` |
| Add skill | `.claude/commands/*.md` or `SkillDefinition::new()` |
| Add subagent | `.claude/agents/*.md` or `SubagentDefinition::new()` |
| Add hook | impl `Hook` trait, `HookManager::register()` |
| Add MCP server | `McpManager::add_server()` with `McpServerConfig` |
| Modify security | `security/` (fs, bash, limits, sandbox) |
| Modify caching | `session/cache.rs` |

## Hook Events (10 types)

| Event | Blockable | Location |
|-------|-----------|----------|
| SessionStart | No | executor.rs |
| UserPromptSubmit | Yes | executor.rs |
| PreToolUse | Yes | executor.rs |
| PostToolUse | No | executor.rs |
| PostToolUseFailure | No | executor.rs |
| PreCompact | No | executor.rs |
| Stop | No | executor.rs |
| SessionEnd | No | executor.rs |
| SubagentStart | No | task.rs |
| SubagentStop | No | task.rs |

## Patterns

```rust
// Tool with auto-schema
impl SchemaTool for MyTool {
    type Input = MyInput;  // JsonSchema + DeserializeOwned
    const NAME: &'static str = "MyTool";
    const DESCRIPTION: &'static str = "...";
    async fn handle(&self, input: Self::Input, ctx: &ExecutionContext) -> ToolResult;
}

// Agent builder
Agent::builder()
    .from_claude_code()           // OAuth from CLI
    .tools(ToolAccess::all())     // 12 tools
    .with_web_search()            // Server tool
    .skill(skill)
    .build()
    .await?;

// ToolRegistry with state
ToolRegistry::builder()
    .tool_state(tool_state)
    .session_id(session_id)
    .build();
```

## Security

- **TOCTOU-safe**: `openat()` + `O_NOFOLLOW`, symlink depth limit
- **Bash AST**: tree-sitter parsing, env sanitization
- **Limits**: `setrlimit()` for CPU, memory, files
- **Sandbox**: Landlock (Linux 5.13+), Seatbelt (macOS)

## Features (Cargo.toml)

```toml
default = ["cli-integration"]
mcp = ["rmcp"]
aws = [...]           # Bedrock
gcp = [...]           # Vertex AI
azure = [...]         # Foundry
postgres = ["sqlx"]
otel = [...]          # OpenTelemetry
full = ["mcp", "cloud-all", "persistence-all", "otel"]
```

## Commands

```bash
cargo test                    # ~1,000 tests
cargo test -- --ignored       # + live API tests
cargo clippy --all-features
```
