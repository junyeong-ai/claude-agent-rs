# CLAUDE.md

## Overview
Production-ready Rust SDK for Claude API. 1100+ tests, zero runtime dependencies.

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
- **`Tool`** trait: 15 tools (12 client + 3 server) + MCP extension

### Module Structure
```
src/
├── agent/          # AgentBuilder, AgentExecutor, AgentConfig
├── auth/           # CredentialProvider chain, OAuth, API Key
├── budget/         # BudgetTracker, TenantBudgetManager, ModelPricing
├── client/         # API client, Provider adapters (Anthropic/Bedrock/Vertex/Foundry)
├── common/         # Provider trait, Index trait, Named trait, SourceType
├── config/         # SandboxSettings, ConfigError, validation
├── context/        # MemoryLoader, ImportExtractor, RuleIndex
├── hooks/          # HookManager, HookEvent (10 types), CommandHook
├── models/         # ModelRegistry, ModelSpec, Pricing, ProviderIds
├── observability/  # MetricsRegistry, TracingConfig, SpanContext
├── output_style/   # OutputStyle, SystemPromptGenerator
├── permissions/    # PermissionPolicy, PermissionMode, PermissionRule
├── prompts/        # BASE_SYSTEM_PROMPT, TOOL_USAGE_POLICY, CODING_INSTRUCTIONS
├── tokens/         # TokenTracker, TokenBudget, ContextWindow, PricingTier
├── types/          # Message, Role, ContentBlock, ToolOutput
├── security/       # SecureFs, Sandbox, BashAnalyzer
├── session/        # Session state, Persistence backends
├── tools/          # 12 client tools (Read, Write, Edit, Bash, etc.)
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
