# claude-agent-rs

Rust SDK for Claude API. 450 tests, 0 clippy warnings.

## Architecture

```
src/
├── auth/           # AuthStrategy pattern (API Key, OAuth, Bedrock, Vertex, Foundry)
├── client/         # HTTP client, streaming, config
├── agent/          # Agent loop, state machine, tool execution
├── tools/          # Tool trait, built-in tools (Read, Write, Edit, Bash, Glob, Grep)
├── skills/         # Skill system (YAML frontmatter, triggers, execution)
├── context/        # Memory loader (CLAUDE.md, @import), static context
├── config/         # Settings loader (settings.json, permissions)
├── mcp/            # MCP server integration (stdio, http, sse)
├── session/        # Session persistence, compaction
├── hooks/          # Pre/post execution hooks
├── permissions/    # Permission modes, tool restrictions
└── prompts/        # System prompts, tool descriptions
```

## Key Patterns

**AuthStrategy** (`src/auth/strategy/traits.rs`): Cloud provider abstraction
- `ApiKeyStrategy`, `OAuthStrategy`, `BedrockStrategy`, `VertexStrategy`, `FoundryStrategy`
- Implement `auth_header()`, `extra_headers()`, `prepare_request()`

**Tool** (`src/tools/mod.rs`): Tool trait
- `name()`, `description()`, `input_schema()`, `execute()`
- Register via `ToolRegistry` or `Agent::builder().tool()`

**SkillDefinition** (`src/skills/mod.rs`): Declarative workflows
- YAML frontmatter: `name`, `description`, `triggers`, `allowed-tools`, `model`
- `$ARGUMENTS`, `$1`-`$9` substitution

## Critical Files

| Task | Files |
|------|-------|
| New cloud provider | `src/auth/strategy/` + `src/client/config.rs` |
| New built-in tool | `src/tools/` + `src/tools/mod.rs` (register) |
| Modify agent loop | `src/agent/mod.rs`, `src/agent/state.rs` |
| Add skill feature | `src/skills/mod.rs`, `src/skills/loader.rs` |
| Change streaming | `src/client/streaming.rs` |

## Commands

```bash
cargo test                    # Run all tests
cargo clippy --all-features   # Lint
cargo doc --open              # Generate docs
```

## Extension Points

**Add Tool**: Implement `Tool` trait, add to `ToolRegistry::default_tools()`

**Add Auth Strategy**: Implement `AuthStrategy`, add to `ClientBuilder`

**Add Skill Feature**: Modify `SkillFrontmatter` in `loader.rs`, update `SkillDefinition`
