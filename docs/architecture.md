# Architecture

claude-agent-rs is a production-ready Rust SDK for Claude API with full Claude Code CLI compatibility.

## Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Application                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────┐    ┌──────────────────────┐                   │
│  │        Agent         │    │        Client        │                   │
│  │  ┌────────────────┐  │    │  ┌────────────────┐  │                   │
│  │  │    Executor    │  │    │  │    Messages    │  │                   │
│  │  │  (agentic loop)│  │    │  │   (streaming)  │  │                   │
│  │  └───────┬────────┘  │    │  └───────┬────────┘  │                   │
│  │          │           │    │          │           │                   │
│  │  ┌───────▼────────┐  │    │  ┌───────▼────────┐  │                   │
│  │  │ ToolRegistry   │  │    │  │ProviderAdapter │  │                   │
│  │  │  (12 tools)    │  │    │  │ (multi-cloud)  │  │                   │
│  │  └────────────────┘  │    │  └────────────────┘  │                   │
│  └──────────────────────┘    └──────────────────────┘                   │
│                                                                          │
├─────────────────────────────────────────────────────────────────────────┤
│                           Supporting Systems                             │
├─────────┬─────────┬─────────┬─────────┬─────────┬─────────┬────────────┤
│  Auth   │ Models  │ Tokens  │ Context │ Session │Security │    MCP     │
│ ┌─────┐ │ ┌─────┐ │ ┌─────┐ │ ┌─────┐ │ ┌─────┐ │ ┌─────┐ │ ┌────────┐ │
│ │OAuth│ │ │Reg  │ │ │Budget│ │ │Mem  │ │ │Cache│ │ │ Fs  │ │ │ Client │ │
│ │Key  │ │ │Spec │ │ │Window│ │ │Rules│ │ │State│ │ │ Bash│ │ │ Manager│ │
│ │Cloud│ │ │Tier │ │ │Track │ │ │Index│ │ │Pers │ │ │ Net │ │ │ Resrc  │ │
│ └─────┘ │ └─────┘ │ └─────┘ │ └─────┘ │ └─────┘ │ └─────┘ │ └────────┘ │
└─────────┴─────────┴─────────┴─────────┴─────────┴─────────┴────────────┘
```

## Core Modules

### Agent (`src/agent/`)

The main execution engine implementing the agentic loop pattern.

| File | Purpose |
|------|---------|
| `executor.rs` | Agent struct with `execute()` and `execute_stream()` |
| `state.rs` | Conversation history and state management |
| `config.rs` | Agent configuration |
| `execution.rs` | Execution loop implementation |
| `request.rs` | Request building |
| `streaming.rs` | Stream processing |
| `events.rs` | Agent event types |
| `common.rs` | Shared utilities |
| `task.rs` | TaskTool for spawning subagents |
| `task_output.rs` | Task output handling |
| `task_registry.rs` | Background task state management |
| `state_formatter.rs` | State formatting utilities |
| `options/` | Builder options (build.rs, builder.rs, cli.rs) |

### Client (`src/client/`)

Low-level API communication with multi-cloud support.

| File | Purpose |
|------|---------|
| `messages/` | Request/Response building (config.rs, context.rs, request.rs) |
| `streaming.rs` | SSE stream processing |
| `adapter/*.rs` | Provider adapters (Anthropic, Bedrock, Vertex, Foundry) |
| `gateway.rs` | Unified gateway pattern |

### Tools (`src/tools/`)

12 built-in tools + 3 server tools with extensible architecture.

| Category | Tools |
|----------|-------|
| File | Read, Write, Edit, Glob, Grep |
| Execution | Bash, KillShell |
| Agent | Task, TaskOutput, TodoWrite, Skill |
| Planning | Plan |

**Server Tools** (Anthropic API): WebFetch, WebSearch, ToolSearch

### Authentication (`src/auth/`)

Flexible credential resolution with automatic refresh.

```
CredentialProvider chain:
  ClaudeCliProvider → EnvironmentProvider → ExplicitProvider
                ↓
         Credential
              ↓
    ┌─────────┴─────────┐
    │                   │
 OAuth              ApiKey
(Bearer token)    (x-api-key)
```

### Security (`src/security/`)

Comprehensive sandboxing with TOCTOU-safe operations.

| Submodule | Purpose |
|-----------|---------|
| `fs/` | `openat()` based file operations, symlink depth limiting |
| `bash/` | AST-based command analysis via tree-sitter |
| `limits/` | Process resource limits via `setrlimit` |
| `path/` | Safe path resolution |
| `sandbox/` | OS-level isolation (Landlock, Seatbelt) |
| `policy/` | Security policy management |

### Skills (`src/skills/`)

Progressive disclosure system for context optimization.

- File-based (`.claude/skills/`, `~/.claude/skills/`)
- Trigger-based activation
- Tool restrictions per skill
- Model overrides

### Context (`src/context/`)

Memory and rules management.

- `memory_loader.rs`: CLAUDE.md loading with `@import`
- `rule_index.rs`: Path-based rule matching
- `orchestrator.rs`: Context assembly
- `level.rs`: LeveledMemoryProvider for multi-level resource aggregation

### Models (`src/models/`)

Model registry with runtime extensibility and pricing tiers.

| File | Purpose |
|------|---------|
| `registry.rs` | Global ModelRegistry with alias resolution |
| `spec.rs` | ModelSpec with capabilities, context limits |
| `family.rs` | ModelFamily (Opus, Sonnet, Haiku), ModelRole |
| `provider.rs` | ProviderIds, ProviderKind |
| `builtin.rs` | Built-in model definitions |

> **Note**: PricingTier is in `src/tokens/tier.rs`, ModelPricing is in `src/budget/pricing.rs`.

**Key Constants:**
- `LONG_CONTEXT_THRESHOLD`: 200,000 tokens (Standard/Extended boundary)
- Extended context: 1M tokens with 2x pricing multiplier

### Tokens (`src/tokens/`)

Token tracking and context window management.

| File | Purpose |
|------|---------|
| `budget.rs` | TokenBudget - separates billing vs context tokens |
| `window.rs` | ContextWindow - tracks usage against model limits |
| `tier.rs` | PricingTier re-export, threshold constants |
| `tracker.rs` | TokenTracker - pre-flight validation via local `check()` |

**Key Concepts:**
- `context_usage() = input_tokens + cache_read_tokens + cache_write_tokens`
- `WindowStatus`: Ok, Warning (80%), Critical (95%), Exceeded
- Pre-flight validation prevents wasted API calls

### Session (`src/session/`)

Prompt caching and conversation management.

- Prompt caching (system + message history)
- Automatic context compaction
- State persistence (Memory/JSONL/PostgreSQL/Redis)

### Output Style (`src/output_style/`)

System prompt customization.

| File | Purpose |
|------|---------|
| `mod.rs` | OutputStyle struct |
| `generator.rs` | SystemPromptGenerator, coding sections assembly |
| `loader.rs` | File-based style loading |
| `builtin.rs` | Default, explanatory, learning styles |

### Prompts (`src/prompts/`)

Modular prompt sections.

- `base.rs`: BASE_SYSTEM_PROMPT, TOOL_USAGE_POLICY, MCP_INSTRUCTIONS
- `coding.rs`: CODING_INSTRUCTIONS, PR_PROTOCOL
- `environment.rs`: Environment detection
- `identity.rs`: CLI_IDENTITY

### Hooks (`src/hooks/`)

Event-driven interception system for agent lifecycle.

| File | Purpose |
|------|---------|
| `traits.rs` | Hook trait, HookEvent (10 types), HookInput/Output |
| `manager.rs` | HookManager with priority-based execution |
| `command.rs` | CommandHook for shell-based hooks |
| `rule.rs` | HookRule, HookAction shared types |

Events: PreToolUse, PostToolUse, UserPromptSubmit, SessionStart/End, SubagentStart/Stop, etc.

### Common (`src/common/`)

Shared traits, utilities, and abstractions used across modules.

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports and re-exports |
| `provider.rs` | `Provider<T>` trait for dependency injection |
| `index.rs` | `Index` trait, `Named` trait for registry patterns |
| `index_loader.rs` | `IndexLoader` trait for loading index entries from config |
| `index_registry.rs` | Generic registry for index-based lookups |
| `source_type.rs` | `SourceType` enum (Builtin, Plugin, User) |
| `tool_matcher.rs` | Tool name matching with glob support |
| `serde_defaults.rs` | Default value helpers for serde deserialization |
| `frontmatter.rs` | YAML frontmatter parsing |
| `content_source.rs` | Content source abstraction |
| `file_provider.rs` | File-based provider utilities |
| `directory.rs` | Directory scanning helpers |
| `path_matched.rs` | Path-matched result wrapper |

### Plugins (`src/plugins/`)

Namespace-based resource management for bundled extensions.

| File | Purpose |
|------|---------|
| `discovery.rs` | PluginDiscovery - scan directories for plugins |
| `manifest.rs` | PluginManifest, PluginDescriptor |
| `loader.rs` | PluginLoader - load skills, subagents, hooks, MCP |
| `manager.rs` | PluginManager - runtime management |
| `namespace.rs` | `plugin-name:resource-name` namespacing |
| `error.rs` | PluginError types |

### MCP (`src/mcp/`)

Model Context Protocol integration for external tools.

| File | Purpose |
|------|---------|
| `client.rs` | McpClient - single server connection (stdio) |
| `manager.rs` | McpManager - multi-server, `mcp__server_tool` naming |
| `resources.rs` | ResourceManager, ResourceQuery |
| `toolset.rs` | McpToolset, McpToolsetRegistry, ToolLoadConfig |

### Subagents (`src/subagents/`)

Independent agents with separate context windows.

| Type | Model | Purpose |
|------|-------|---------|
| `Bash` | Small (Haiku) | Command execution |
| `Explore` | Small (Haiku) | Fast codebase search |
| `Plan` | Primary (Sonnet) | Implementation planning |
| `general-purpose` | Primary (Sonnet) | Complex multi-step tasks |

- Context isolation: Clean context for task-specific execution
- Background execution: Non-blocking async tasks
- Tool restrictions: Security through capability limiting

### Config (`src/config/`)

Settings management and configuration loading.

### Types (`src/types/`)

Shared type definitions and API response structures.

## Data Flow

```
User Request
    │
    ▼
┌─────────────────────┐
│ Agent.execute()     │
│   ├── Load context  │ ← CLAUDE.md, skills, rules
│   ├── Build prompt  │ ← System prompt + tools
│   └── Start loop    │
└─────────┬───────────┘
          │
          ▼
    ┌───────────┐
    │ API Call  │ ← Client → ProviderAdapter
    └─────┬─────┘
          │
          ▼
    ┌───────────────┐
    │ Tool Execution│ ← ToolRegistry → Security checks
    └───────┬───────┘
          │
          ▼
    Loop until stop_reason != "tool_use"
          │
          ▼
    Final Response
```

## Extension Points

| Task | Implementation |
|------|----------------|
| Add tool | Implement `Tool` trait, register in `ToolRegistry` |
| Add auth provider | Implement `CredentialProvider`, add to chain |
| Add skill | Create `.md` file or use `SkillIndex::new()` |
| Add subagent | Create `.md` file or use `SubagentIndex::new()` |
| Add hook | Implement `Hook` trait, register via `HookManager::register()` |
| Add MCP server | `McpManager::add_server()` with `McpServerConfig` |
| Add plugin | Create `.claude-plugin/plugin.json` + resources |
| Custom output style | Create `.claude/output-styles/*.md` |

## Key Design Decisions

1. **Blanket impl for SchemaTool**: Auto-generates schema from `schemars`
2. **Builder pattern**: Async for Agent, sync for Client
3. **Strategy pattern**: ProviderAdapter for multi-cloud
4. **Registry pattern**: Centralized tool/skill/subagent management
5. **TOCTOU-safe**: All file operations use `openat()`
