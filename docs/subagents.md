# Subagents System

Subagents are independent agents that execute in separate context windows.

> **Related**: [Skills Guide](skills.md) for reusable workflows with similar configuration options (tools, model override)

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Main Agent                              │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                  Conversation                          │  │
│  │  User: "Explore the codebase and plan the feature"    │  │
│  └──────────────────────────┬────────────────────────────┘  │
│                             │                                │
│                             ▼                                │
│                    ┌─────────────────┐                       │
│                    │   Task Tool     │                       │
│                    └────────┬────────┘                       │
│                             │                                │
│         ┌───────────────────┼───────────────────┐            │
│         ▼                   ▼                   ▼            │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │
│  │   explore   │    │    plan     │    │  general-   │      │
│  │  subagent   │    │  subagent   │    │  purpose    │      │
│  │             │    │             │    │             │      │
│  │ Tools:      │    │ Tools:      │    │ Tools:      │      │
│  │ Read,Grep,  │    │ All         │    │ All         │      │
│  │ Glob,Bash   │    │             │    │             │      │
│  │             │    │             │    │             │      │
│  │ Model:      │    │ Model:      │    │ Model:      │      │
│  │ haiku       │    │ sonnet      │    │ sonnet      │      │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘      │
│         │                  │                  │              │
│         └──────────────────┴──────────────────┘              │
│                            │                                 │
│                            ▼                                 │
│                    Result to Main Agent                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Built-in Subagents

### Bash

Command execution specialist.

| Property | Value |
|----------|-------|
| Tools | Bash |
| Model | Haiku (Small) |
| Use case | Command execution |

### Explore

Fast agent for codebase exploration.

| Property | Value |
|----------|-------|
| Tools | Read, Grep, Glob, Bash, TodoWrite, KillShell |
| Model | Haiku (Small) |
| Use case | Quick file/code search |

### Plan

Software architect for implementation planning.

| Property | Value |
|----------|-------|
| Tools | Read, Grep, Glob, Bash, TodoWrite, KillShell |
| Model | Sonnet (Primary) |
| Use case | Design and planning |

### general-purpose

Full capability agent for complex tasks.

| Property | Value |
|----------|-------|
| Tools | All |
| Model | Sonnet (Primary) |
| Use case | Multi-step autonomous tasks |

## Subagent Definition

### Programmatic

```rust
use claude_agent::{SubagentIndex, ContentSource};

let mut subagent = SubagentIndex::new("code-reviewer", "Specialized code review agent")
    .source(ContentSource::in_memory(r#"
You are a code review specialist.
1. Analyze the code for bugs and security issues
2. Check coding standards compliance
3. Suggest improvements
4. Return a structured review report
    "#))
    .allowed_tools(["Read", "Grep", "Glob"])
    .model("claude-sonnet-4-5-20250929");
subagent.permission_mode = Some("plan".to_string());
```

### File-based

Create `.claude/agents/code-reviewer.md`:

```markdown
---
name: code-reviewer
description: Specialized code review agent
tools:
  - Read
  - Grep
  - Glob
model: claude-sonnet-4-5-20250929
permission-mode: plan
---

You are a code review specialist.
1. Analyze the code for bugs and security issues
2. Check coding standards compliance
3. Suggest improvements
4. Return a structured review report
```

## Configuration Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `name` | string | (required) | Unique identifier |
| `description` | string | (required) | Brief description |
| `tools` | string | — | Comma-separated allowed tools |
| `model` | string | — | Model override |
| `model_type` | string | — | Model type hint |
| `skills` | string | — | Comma-separated available skills |
| `permission-mode` | string | — | Permission level (see below) |
| `disallowedTools` | string | — | Comma-separated blocked tools |
| `source-type` | string | — | Source type (Builtin, Project, Managed, Plugin) |
| `hooks` | object | — | Lifecycle hooks (map of event → `HookRule[]`) |

## Usage via Task Tool

```rust
// From main agent's perspective
{
    "description": "Review authentication code",
    "prompt": "Review the auth module for security issues",
    "subagent_type": "code-reviewer",
    "model": "claude-sonnet-4-5-20250929",  // optional override
    "run_in_background": false
}
```

## Background Execution

Run subagents asynchronously:

```rust
// Start in background
{
    "description": "Long running analysis",
    "prompt": "Analyze entire codebase",
    "subagent_type": "explore",
    "run_in_background": true
}

// Later, retrieve result
{
    "task_id": "task-abc123",
    "block": true,
    "timeout": 30000
}
```

## Resuming Subagents

Resume a previous subagent with context:

```rust
{
    "description": "Continue analysis",
    "prompt": "Now check the test files too",
    "subagent_type": "explore",
    "resume": "agent-xyz789"  // Previous agent ID
}
```

## Tool Restrictions

Subagents can limit available tools:

```rust
let subagent = SubagentIndex::new("reader", "Read-only agent")
    .source(ContentSource::in_memory("..."))
    .allowed_tools(["Read", "Glob", "Grep"]);

// Write, Bash, etc. are not available
```

Pattern-based restrictions:

```rust
.tools(["Bash(git:*)", "Read"])
// Bash only for git commands
```

## Permission Modes

| Mode | Aliases | Description |
|------|---------|-------------|
| `bypassPermissions` | `bypass` | All operations allowed |
| `plan` | `readonly`, `read-only` | Read-only tools only |
| `acceptEdits` | `accept-edits` | Auto-approve file operations |
| `default` | — | Standard permission flow with allow/deny rules |

```rust
let mut subagent = SubagentIndex::new("safe-agent", "Safe agent")
    .source(ContentSource::in_memory("..."));
subagent.permission_mode = Some("plan".to_string());  // Read-only
```

## Registration

```rust
use claude_agent::common::IndexRegistry;
use claude_agent::subagents::SubagentIndex;

let mut registry = IndexRegistry::<SubagentIndex>::new();
registry.register(code_reviewer);
registry.register(security_auditor);

let agent = Agent::builder()
    .from_claude_code(".").await?
    .subagent_registry(registry)
    .build()
    .await?;
```

## Context Isolation

Each subagent runs in a separate context:

- **Clean context**: No pollution from main conversation
- **Task-specific**: Only relevant information
- **Independent iteration**: Own agentic loop
- **Result return**: Single response to main agent

## Best Practices

1. **Use explore for search**: Fast haiku model for quick lookups
2. **Use plan for design**: Stronger model for architectural decisions
3. **Background for long tasks**: Don't block main conversation
4. **Limit tools appropriately**: Security through restriction
5. **Clear prompts**: Subagents need complete context
