# CLAUDE.md

## Overview
Production-ready Rust SDK for Claude API. Comprehensive test suite, no CLI dependency required.

## Commands
```bash
cargo build --release
cargo nextest run --all-features
cargo clippy --all-features -- -D warnings
cargo fmt --all -- --check
```

## Architecture

### Core Patterns
- **`Provider<T>`** trait: Generic loading pattern for named items (output styles, etc.)
- **`Persistence`** trait: Memory, JSONL, PostgreSQL, Redis backends
- **`Tool`** trait: 15 tools (12 client + 3 server) + MCP extension

### Module Structure
```
src/
├── agent/          # AgentBuilder, Agent, AgentConfig
├── auth/           # CredentialProvider chain, OAuth, API Key
├── budget/         # BudgetTracker, TenantBudgetManager, ModelPricing
├── client/         # API client, Provider adapters (Anthropic/Bedrock/Vertex/Foundry)
├── common/         # Provider trait, Index trait, Named trait, SourceType, ContentSource, IndexRegistry, ToolRestricted trait
├── config/         # SandboxSettings, ConfigError, validation
├── context/        # MemoryLoader, ImportExtractor, RuleIndex
├── hooks/          # HookManager, HookEvent (10 types), CommandHook, HookRule, HookAction
├── models/         # ModelRegistry, ModelSpec, ProviderIds, ProviderKind
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
├── plugins/        # PluginManager, PluginLoader, PluginDiscovery, namespace
├── skills/         # Skill loader and execution
└── subagents/      # Subagent spawning (Bash, Explore, Plan, general-purpose)
```

### Security Layer (`src/security/`)
- **SecureFs**: TOCTOU-safe via `openat()` + `O_NOFOLLOW`
- **Sandbox**: Landlock (Linux), Seatbelt (macOS)
- **BashAnalyzer**: tree-sitter AST command analysis

### Session Layer (`src/session/`)
- **Persistence backends**: Memory (default), JSONL, PostgreSQL, Redis
- **Change detection**: Hash-based incremental writes (JSONL backend)
- **JSONL format**: CLI-compatible `~/.claude/projects/`

## Conventions
- `tokio` async runtime
- `serde` JSON serialization
- `Result<T, E>` error handling
- Feature flags for optional dependencies
