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
| `context.rs` | Token estimation and context management |
| `task.rs` | TaskTool for spawning subagents |
| `task_registry.rs` | Background task state management |

### Client (`src/client/`)

Low-level API communication with multi-cloud support.

| File | Purpose |
|------|---------|
| `messages.rs` | Request/Response building |
| `streaming.rs` | SSE stream processing |
| `adapter/*.rs` | Provider adapters (Anthropic, Bedrock, Vertex, Foundry) |
| `gateway.rs` | Unified gateway pattern |

### Tools (`src/tools/`)

12 built-in tools + 2 server tools with extensible architecture.

| Category | Tools |
|----------|-------|
| File | Read, Write, Edit, Glob, Grep |
| Execution | Bash, KillShell |
| Agent | Task, TaskOutput, TodoWrite, Skill |
| Planning | Plan |

**Server Tools** (Anthropic API): WebFetch, WebSearch

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
| `pricing.rs` | PricingTier (Standard/Extended), cost calculation |
| `builtin.rs` | Built-in model definitions |

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
| `tracker.rs` | TokenTracker - pre-flight validation via `count_tokens` API |

**Key Concepts:**
- `context_usage() = input_tokens + cache_read_tokens + cache_write_tokens`
- `WindowStatus`: Ok, Warning (80%), Critical (95%), Exceeded
- Pre-flight validation prevents wasted API calls

### Session (`src/session/`)

Prompt caching and conversation management.

- Prompt caching (system + message history)
- Automatic context compaction
- State persistence (Memory/PostgreSQL/Redis)

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

- `sections.rs`: BASE_IDENTIFIER, CORE_PRINCIPLES, TOOL_USAGE, SAFETY_RULES

### Hooks (`src/hooks/`)

Event-driven interception system for agent lifecycle.

| File | Purpose |
|------|---------|
| `traits.rs` | Hook trait, HookEvent (10 types), HookInput/Output |
| `manager.rs` | HookManager with priority-based execution |
| `command.rs` | CommandHook for shell-based hooks |

Events: PreToolUse, PostToolUse, UserPromptSubmit, SessionStart/End, SubagentStart/Stop, etc.

### MCP (`src/mcp/`)

Model Context Protocol integration for external tools.

| File | Purpose |
|------|---------|
| `client.rs` | McpClient - single server connection (stdio, SSE) |
| `manager.rs` | McpManager - multi-server, `mcp__server_tool` naming |
| `resources.rs` | ResourceManager, ResourceQuery |

### Subagents (`src/subagents/`)

Independent agents with separate context windows.

| Type | Model | Purpose |
|------|-------|---------|
| `explore` | Haiku | Fast codebase search |
| `plan` | Sonnet | Implementation planning |
| `general` | Sonnet | Complex multi-step tasks |

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
| Custom output style | Create `.claude/output-styles/*.md` |

## Key Design Decisions

1. **Blanket impl for SchemaTool**: Auto-generates schema from `schemars`
2. **Builder pattern**: Async for Agent, sync for Client
3. **Strategy pattern**: ProviderAdapter for multi-cloud
4. **Registry pattern**: Centralized tool/skill/subagent management
5. **TOCTOU-safe**: All file operations use `openat()`
