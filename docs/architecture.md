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
├──────────┬──────────┬──────────┬──────────┬──────────┬─────────────────┤
│  Auth    │  Skills  │ Context  │ Session  │ Security │      MCP        │
│ ┌──────┐ │ ┌──────┐ │ ┌──────┐ │ ┌──────┐ │ ┌──────┐ │ ┌─────────────┐ │
│ │OAuth │ │ │Loader│ │ │Memory│ │ │Cache │ │ │ Fs   │ │ │   Client    │ │
│ │APIKey│ │ │Exec  │ │ │Rules │ │ │State │ │ │ Bash │ │ │   Manager   │ │
│ │Cloud │ │ │Tool  │ │ │Index │ │ │Compact│ │ │Network│ │ │   Resources │ │
│ └──────┘ │ └──────┘ │ └──────┘ │ └──────┘ │ └──────┘ │ └─────────────┘ │
└──────────┴──────────┴──────────┴──────────┴──────────┴─────────────────┘
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

### Session (`src/session/`)

Prompt caching and conversation management.

- Cache statistics tracking
- Automatic context compaction
- State persistence

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
| `traits.rs` | Hook trait, HookEvent (12 types), HookInput/Output |
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
| Add skill | Create `.md` file or use `SkillDefinition::new()` |
| Add subagent | Create `.md` file or use `SubagentDefinition::new()` |
| Add hook | Implement `Hook` trait, register via `HookManager::register()` |
| Add MCP server | `McpManager::add_server()` with `McpServerConfig` |
| Custom output style | Create `.claude/output-styles/*.md` |

## Key Design Decisions

1. **Blanket impl for SchemaTool**: Auto-generates schema from `schemars`
2. **Builder pattern**: Async for Agent, sync for Client
3. **Strategy pattern**: ProviderAdapter for multi-cloud
4. **Registry pattern**: Centralized tool/skill/subagent management
5. **TOCTOU-safe**: All file operations use `openat()`
