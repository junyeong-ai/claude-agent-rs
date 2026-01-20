# Permissions System

The permissions system controls tool execution through modes, rules, and limits.

> **Related**: [Security Guide](security.md) for TOCTOU-safe operations | [Sandbox Guide](sandbox.md) for OS-level isolation

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                   Permission Check Flow                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Tool Request (name, input)                                  │
│       │                                                      │
│       ▼                                                      │
│  ┌────────────────────┐                                      │
│  │  Mode Check        │ BypassPermissions → Allow All        │
│  │  (first filter)    │                                      │
│  └─────────┬──────────┘                                      │
│            │                                                 │
│            ▼                                                 │
│  ┌────────────────────┐                                      │
│  │  Deny Rules        │ Match → Denied                       │
│  │  (highest priority)│                                      │
│  └─────────┬──────────┘                                      │
│            │                                                 │
│            ▼                                                 │
│  ┌────────────────────┐                                      │
│  │  Allow Rules       │ Match → Allowed                      │
│  │  (explicit allow)  │                                      │
│  └─────────┬──────────┘                                      │
│            │                                                 │
│            ▼                                                 │
│  ┌────────────────────┐                                      │
│  │  Mode Default      │ Plan → Read-only                     │
│  │  (fallback)        │ AcceptEdits → File tools             │
│  │                    │ Default → Denied                     │
│  └────────────────────┘                                      │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Permission Modes

| Mode | Description | Default Allow |
|------|-------------|---------------|
| `BypassPermissions` | All tools allowed | Everything |
| `Plan` | Read-only mode | Read, Glob, Grep, WebSearch, WebFetch |
| `AcceptEdits` | File operations | Read, Write, Edit, Glob, Grep |
| `Default` | Explicit rules only | Nothing (must specify rules) |

### Mode Usage

```rust
use claude_agent::PermissionMode;

// Permissive (development)
let policy = PermissionPolicy::permissive();

// Read-only (safe exploration)
let policy = PermissionPolicy::read_only();

// Accept edits (code modification)
let policy = PermissionPolicy::accept_edits();

// Custom with builder
let policy = PermissionPolicy::builder()
    .mode(PermissionMode::Default)
    .allow("Read")
    .allow("Bash(git:*)")
    .deny("Write(/etc/*)")
    .build();
```

## Permission Rules

Rules match tools by name and optionally by input patterns.

### Simple Rules

```rust
// Allow specific tools
.allow("Read")
.allow("Write")
.allow("Bash")

// Deny specific tools
.deny("KillShell")
.deny("WebFetch")
```

### Regex Patterns

```rust
// Allow multiple tools with regex
.allow("Read|Write|Edit")

// Allow tools matching pattern
.allow("Web.*")
```

### Scoped Rules

Match tool input parameters:

```rust
// Bash only for git commands
.allow("Bash(git:*)")

// Read only from specific directory
.allow("Read(/project/src/**)")

// WebFetch only from specific domain
.allow("WebFetch(domain:github.com)")

// Deny writes to sensitive paths
.deny("Write(/etc/*)")
.deny("Edit(*.env)")
```

### Scope Patterns

| Tool | Scoped Field | Example |
|------|--------------|---------|
| Bash | command | `Bash(git:*)`, `Bash(npm:*)` |
| Read, Write, Edit | file_path | `Read(/src/**)` |
| Glob, Grep | path | `Grep(/src/**)` |
| WebFetch | url / domain: | `WebFetch(domain:github.com)` |

## Rule Priority

1. **Deny rules** - Always checked first, highest priority
2. **Allow rules** - Checked if no deny matches
3. **Mode default** - Fallback if no rules match

```rust
let policy = PermissionPolicy::builder()
    .mode(PermissionMode::AcceptEdits)  // Base: file tools allowed
    .deny("Write")                       // Override: deny Write
    .build();

// Read, Edit, Glob, Grep allowed
// Write denied (explicit rule)
// Bash denied (mode default)
```

## Tool Limits

Configure resource constraints per tool:

```rust
use claude_agent::ToolLimits;

let policy = PermissionPolicy::builder()
    .tool_limits("Bash", ToolLimits::with_timeout(30_000))
    .tool_limits("Read", ToolLimits::with_max_output(100_000))
    .build();
```

### Limit Options

| Field | Type | Description |
|-------|------|-------------|
| `timeout_ms` | u64 | Execution timeout |
| `max_output_size` | usize | Output truncation limit |
| `max_concurrent` | usize | Concurrent execution limit |
| `allowed_paths` | Vec<String> | Path whitelist |
| `denied_paths` | Vec<String> | Path blacklist |

### Path Limits

```rust
let limits = ToolLimits::default()
    .with_allowed_paths(vec!["/project/src".into()])
    .with_denied_paths(vec!["/project/.env".into()]);
```

## PermissionPolicy API

```rust
use claude_agent::{PermissionPolicy, PermissionMode, ToolLimits};

let policy = PermissionPolicy::builder()
    // Set base mode
    .mode(PermissionMode::Default)

    // Allow rules
    .allow("Read")
    .allow("Grep")
    .allow("Bash(git:*)")

    // Deny rules
    .deny("Write(*.env)")
    .deny("Bash(rm:*)")

    // Tool limits
    .tool_limits("Bash", ToolLimits::with_timeout(60_000))

    .build();
```

## Checking Permissions

```rust
let result = policy.check("Bash", &serde_json::json!({
    "command": "git status"
}));

if result.is_allowed() {
    println!("Allowed: {}", result.reason);
} else {
    println!("Denied: {}", result.reason);
}
```

## Integration with Agent

```rust
let agent = Agent::builder()
    .from_claude_code(".").await?
    .permission_policy(policy)
    .build()
    .await?;
```

## Tool Categories

Helper functions for tool classification:

```rust
use claude_agent::permissions::{is_read_only_tool, is_file_tool, is_shell_tool};

// Read-only tools
is_read_only_tool("Read")      // true
is_read_only_tool("Grep")      // true
is_read_only_tool("Write")     // false

// File operation tools
is_file_tool("Write")          // true
is_file_tool("Edit")           // true
is_file_tool("Bash")           // false

// Shell/execution tools
is_shell_tool("Bash")          // true
is_shell_tool("KillShell")     // true
is_shell_tool("Read")          // false
```

## Security Considerations

1. **Use restrictive modes by default**: Start with `Default` or `Plan`
2. **Deny sensitive operations**: Explicitly deny dangerous patterns
3. **Scope Bash commands**: Use `Bash(git:*)` instead of broad `Bash`
4. **Set timeouts**: Prevent runaway processes
5. **Path restrictions**: Limit file access to project directories
