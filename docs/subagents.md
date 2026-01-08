# Subagents System

Subagents are independent agents that execute in separate context windows.

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
│  │   explore   │    │    plan     │    │  general    │      │
│  │  subagent   │    │  subagent   │    │  subagent   │      │
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

### explore

Fast agent for codebase exploration.

| Property | Value |
|----------|-------|
| Tools | Read, Grep, Glob, Bash |
| Model | Haiku (Small) |
| Use case | Quick file/code search |

### plan

Software architect for implementation planning.

| Property | Value |
|----------|-------|
| Tools | All |
| Model | Sonnet (Primary) |
| Use case | Design and planning |

### general

Full capability agent for complex tasks.

| Property | Value |
|----------|-------|
| Tools | All |
| Model | Sonnet (Primary) |
| Use case | Multi-step autonomous tasks |

## Subagent Definition

### Programmatic

```rust
use claude_agent::SubagentDefinition;

let subagent = SubagentDefinition::new(
    "code-reviewer",
    "Specialized code review agent",
    r#"
You are a code review specialist.
1. Analyze the code for bugs and security issues
2. Check coding standards compliance
3. Suggest improvements
4. Return a structured review report
    "#,
)
.with_tools(["Read", "Grep", "Glob"])
.with_model("claude-sonnet-4-20250514")
.with_permission_mode("plan");
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
model: claude-sonnet-4-20250514
permission-mode: plan
---

You are a code review specialist.
1. Analyze the code for bugs and security issues
2. Check coding standards compliance
3. Suggest improvements
4. Return a structured review report
```

## Configuration Options

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Unique identifier |
| `description` | string | Brief description |
| `prompt` | string | Agent instructions |
| `tools` | array | Allowed tools |
| `model` | string | Model override |
| `permission-mode` | string | Permission level |
| `skills` | array | Available skills |

## Usage via Task Tool

```rust
// From main agent's perspective
{
    "description": "Review authentication code",
    "prompt": "Review the auth module for security issues",
    "subagent_type": "code-reviewer",
    "model": "claude-sonnet-4-20250514",  // optional override
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
let subagent = SubagentDefinition::new("reader", "Read-only agent", "...")
    .with_tools(["Read", "Glob", "Grep"]);

// Write, Bash, etc. are not available
```

Pattern-based restrictions:

```rust
.with_tools(["Bash(git:*)", "Read"])
// Bash only for git commands
```

## Permission Modes

| Mode | Description |
|------|-------------|
| `bypass` | All operations allowed |
| `plan` | Read-only tools only |
| `accept-edits` | File tools only |
| `default` | Explicit rules required |

```rust
let subagent = SubagentDefinition::new("safe-agent", "Safe agent", "...")
    .with_permission_mode("plan");  // Read-only
```

## Registration

```rust
use claude_agent::SubagentRegistry;

let mut registry = SubagentRegistry::new();
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
