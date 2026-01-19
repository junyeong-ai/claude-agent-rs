# CLAUDE.md

## Overview
Production-ready Rust SDK for Claude API. 1000+ tests, zero runtime dependencies.

## Commands
```bash
cargo build --release
cargo nextest run --all-features
cargo clippy --all-features -- -D warnings
cargo fmt --all -- --check
```

## Architecture

### Core Patterns
- **`Provider<T>`** trait: Separates Auth, Resources, Settings providers
- **`Persistence`** trait: Memory, JSONL, PostgreSQL, Redis backends
- **`Tool`** trait: 12 built-in tools + MCP extension

### Module Structure
```
src/
├── agent/          # AgentBuilder, AgentExecutor, HookManager
├── client/         # API client, Provider adapters (Anthropic/Bedrock/Vertex/Foundry)
├── context/        # MemoryLoader, ImportExtractor, RuleIndex
├── security/       # SecureFs, Sandbox, BashAnalyzer
├── session/        # Session state, Persistence backends
├── tools/          # 12 built-in tools (Read, Write, Bash, etc.)
├── mcp/            # MCP client integration
├── skills/         # Skill loader and execution
└── subagents/      # Subagent spawning (explore, plan, general)
```

### Security Layer (`src/security/`)
- **SecureFs**: TOCTOU-safe via `openat()` + `O_NOFOLLOW`
- **Sandbox**: Landlock (Linux), Seatbelt (macOS)
- **BashAnalyzer**: tree-sitter AST command analysis

### Session Layer (`src/session/`)
- **Persistence backends**: Memory (default), JSONL, PostgreSQL, Redis
- **Change detection**: Hash-based dirty tracking
- **JSONL format**: CLI-compatible `~/.claude/projects/`

## Conventions
- `tokio` async runtime
- `serde` JSON serialization
- `Result<T, E>` error handling
- Feature flags for optional dependencies
