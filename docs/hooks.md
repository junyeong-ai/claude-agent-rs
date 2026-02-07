# Hooks System

The hooks system allows intercepting and controlling agent execution at specific points in the lifecycle.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      Hook Execution Flow                     │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌──────────┐     ┌──────────┐     ┌──────────┐           │
│   │  Event   │────▶│  Hooks   │────▶│  Output  │           │
│   │ Trigger  │     │ (sorted) │     │  Merge   │           │
│   └──────────┘     └──────────┘     └──────────┘           │
│                          │                │                 │
│                          │                │                 │
│                          ▼                ▼                 │
│                    ┌──────────┐     ┌──────────┐           │
│                    │ Priority │     │ Continue │           │
│                    │  Order   │     │ or Block │           │
│                    └──────────┘     └──────────┘           │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Hook Events

10 event types for different lifecycle points:

| Event | Description | Can Block | Location |
|-------|-------------|-----------|----------|
| `PreToolUse` | Before tool execution | Yes | executor.rs |
| `PostToolUse` | After successful execution | No | executor.rs |
| `PostToolUseFailure` | After failed execution | No | executor.rs |
| `UserPromptSubmit` | When user submits prompt | Yes | executor.rs |
| `Stop` | When agent stops | No | executor.rs |
| `SubagentStart` | When subagent spawns | Yes | task.rs |
| `SubagentStop` | When subagent completes | No | task.rs |
| `PreCompact` | Before context compaction | No | executor.rs |
| `SessionStart` | When session begins | Yes | executor.rs |
| `SessionEnd` | When session ends | No | executor.rs |

## Core Types

### Hook Trait

```rust
#[async_trait]
pub trait Hook: Send + Sync {
    fn name(&self) -> &str;
    fn events(&self) -> &[HookEvent];
    fn tool_matcher(&self) -> Option<&Regex> { None }  // Optional regex filter
    fn timeout_secs(&self) -> u64 { 60 }
    fn priority(&self) -> i32 { 0 }  // Higher = runs first
    async fn execute(&self, input: HookInput, ctx: &HookContext) -> Result<HookOutput>;
}
```

### HookEventData (Type-Safe Event Data)

```rust
pub enum HookEventData {
    PreToolUse { tool_name: String, tool_input: Value },
    PostToolUse { tool_name: String, tool_result: ToolOutput },
    PostToolUseFailure { tool_name: String, error: String },
    UserPromptSubmit { prompt: String },
    Stop,
    SubagentStart { subagent_id: String, subagent_type: String, description: String },
    SubagentStop { subagent_id: String, success: bool, error: Option<String> },
    PreCompact,
    SessionStart,
    SessionEnd,
}
```

### HookInput

Wrapper providing session context and timestamp:

```rust
pub struct HookInput {
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub data: HookEventData,
    pub metadata: Option<Value>,
}
```

Factory methods:
- `HookInput::pre_tool_use(session_id, tool_name, input)`
- `HookInput::post_tool_use(session_id, tool_name, result)`
- `HookInput::post_tool_use_failure(session_id, tool_name, error)`
- `HookInput::user_prompt_submit(session_id, prompt)`
- `HookInput::session_start(session_id)`
- `HookInput::session_end(session_id)`
- `HookInput::stop(session_id)`
- `HookInput::pre_compact(session_id)`
- `HookInput::subagent_start(session_id, subagent_id, subagent_type, description)`
- `HookInput::subagent_stop(session_id, subagent_id, success, error)`

Accessor methods:
- `event_type()` - Get the HookEvent enum variant
- `tool_name()` - Get tool name for tool-related events
- `subagent_id()` - Get subagent ID for subagent events

### HookOutput

Response from hook execution:

```rust
pub struct HookOutput {
    pub continue_execution: bool,         // false = block
    pub stop_reason: Option<String>,
    pub suppress_logging: bool,
    pub system_message: Option<String>,   // Inject into context
    pub updated_input: Option<Value>,     // Modified tool input
    pub additional_context: Option<String>,
}
```

Factory methods:
- `HookOutput::allow()` - Continue execution
- `HookOutput::block(reason)` - Stop execution

Builder methods:
- `.system_message(msg)` - Inject system message
- `.context(ctx)` - Add additional context
- `.updated_input(input)` - Modify tool input
- `.suppress_logging()` - Suppress logging

### HookContext

Execution context provided to hooks:

```rust
pub struct HookContext {
    pub session_id: String,
    pub cancellation_token: CancellationToken,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
}
```

## HookManager

Manages hook registration and execution.

```rust
let mut manager = HookManager::new();

// Register hooks
manager.register(SecurityHook::new());
manager.register_arc(Arc::new(LoggingHook));

// Execute hooks for event
let input = HookInput::pre_tool_use("session-1", "Bash", json!({"command": "ls"}));
let ctx = HookContext::new("session-1");
let output = manager.execute(HookEvent::PreToolUse, input, &ctx).await?;

if !output.continue_execution {
    println!("Blocked: {:?}", output.stop_reason);
}
```

### Execution Order

1. Hooks sorted by priority (higher first)
2. Tool matcher checked (if defined)
3. Execute with timeout
4. Merge outputs (any block = stop)
5. Return merged result

### Output Merging Rules

| Field | Merge Strategy |
|-------|---------------|
| `continue_execution` | AND (any false = false) |
| `stop_reason` | Latest non-None |
| `suppress_logging` | OR (any true = true) |
| `system_message` | Latest non-None |
| `updated_input` | Latest non-None |
| `additional_context` | Concatenate with newline |

## CommandHook

Shell command-based hooks from settings.

```rust
pub struct CommandHook {
    name: String,
    command: String,
    events: Vec<HookEvent>,
    tool_pattern: Option<Regex>,
    timeout_secs: u64,
    extra_env: HashMap<String, String>,
}
```

### Configuration

In `~/.claude/settings.json` or `.claude/settings.json`:

```json
{
  "hooks": {
    "preToolUse": {
      "security-check": {
        "command": "python check_security.py",
        "timeout_secs": 30,
        "matcher": "Bash|Write"
      }
    },
    "postToolUse": {
      "audit-log": "echo '{\"event\":\"$EVENT\"}' >> /tmp/audit.log"
    },
    "sessionStart": [
      "notify-send 'Session started'"
    ],
    "sessionEnd": [
      "cleanup-temp.sh"
    ]
  }
}
```

### Command Interface

**Input (stdin)**: JSON payload

```json
{
  "event": "pre_tool_use",
  "session_id": "abc-123",
  "tool_name": "Bash",
  "tool_input": {"command": "rm -rf /"}
}
```

**Output (stdout)**: JSON response (optional)

```json
{
  "continue_execution": false,
  "stop_reason": "Dangerous command blocked",
  "updated_input": null
}
```

Empty or no output = allow execution.

## Usage Example

### Custom Security Hook

```rust
use claude_agent::hooks::{Hook, HookEvent, HookInput, HookOutput, HookContext};
use async_trait::async_trait;

struct SecurityHook {
    blocked_patterns: Vec<String>,
}

#[async_trait]
impl Hook for SecurityHook {
    fn name(&self) -> &str { "security-hook" }

    fn events(&self) -> &[HookEvent] {
        &[HookEvent::PreToolUse]
    }

    fn priority(&self) -> i32 { 100 }  // Run early

    async fn execute(&self, input: HookInput, _ctx: &HookContext)
        -> Result<HookOutput, claude_agent::Error>
    {
        if let Some(tool_name) = input.tool_name() {
            if tool_name == "Bash" {
                if let Some(tool_input) = input.data.tool_input() {
                    if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
                        for pattern in &self.blocked_patterns {
                            if cmd.contains(pattern) {
                                return Ok(HookOutput::block(
                                    format!("Blocked: contains '{}'", pattern)
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(HookOutput::allow())
    }
}

// Usage
let mut manager = HookManager::new();
manager.register(SecurityHook {
    blocked_patterns: vec!["rm -rf".into(), "sudo".into()],
});
```

### Logging Hook

```rust
struct LoggingHook;

#[async_trait]
impl Hook for LoggingHook {
    fn name(&self) -> &str { "logging-hook" }

    fn events(&self) -> &[HookEvent] {
        &[HookEvent::PreToolUse, HookEvent::PostToolUse, HookEvent::PostToolUseFailure]
    }

    async fn execute(&self, input: HookInput, _ctx: &HookContext)
        -> Result<HookOutput, claude_agent::Error>
    {
        let event = input.event_type().to_string();
        let tool = input.tool_name().unwrap_or_default();

        tracing::info!(event = %event, tool = %tool, "Hook event");

        Ok(HookOutput::allow())
    }
}
```

### Subagent Monitoring Hook

```rust
struct SubagentMonitorHook;

#[async_trait]
impl Hook for SubagentMonitorHook {
    fn name(&self) -> &str { "subagent-monitor" }

    fn events(&self) -> &[HookEvent] {
        &[HookEvent::SubagentStart, HookEvent::SubagentStop]
    }

    async fn execute(&self, input: HookInput, _ctx: &HookContext)
        -> Result<HookOutput, claude_agent::Error>
    {
        match &input.data {
            HookEventData::SubagentStart { subagent_id, subagent_type, description } => {
                tracing::info!(
                    id = %subagent_id,
                    type_ = %subagent_type,
                    desc = %description,
                    "Subagent started"
                );
            }
            HookEventData::SubagentStop { subagent_id, success, error } => {
                if *success {
                    tracing::info!(id = %subagent_id, "Subagent completed successfully");
                } else {
                    tracing::warn!(
                        id = %subagent_id,
                        error = ?error,
                        "Subagent failed"
                    );
                }
            }
            _ => {}
        }
        Ok(HookOutput::allow())
    }
}
```

## Integration with Agent

```rust
use claude_agent::Agent;

let agent = Agent::builder()
    .from_claude_code(".").await?
    .hook(SecurityHook::new())
    .hook(LoggingHook)
    .build()
    .await?;
```

## ExecutionContext Integration

For tools that need to fire hooks (like TaskTool for subagent events):

```rust
// ExecutionContext with hooks support
let ctx = ExecutionContext::new(security)
    .hooks(hook_manager, session_id);

// Fire hook from within a tool
ctx.fire_hook(
    HookEvent::SubagentStart,
    HookInput::subagent_start(&session_id, &agent_id, &agent_type, &description),
).await;
```

## File Locations

| Type | File |
|------|------|
| `HookEvent` | hooks/traits.rs |
| `HookEventData` | hooks/traits.rs |
| `HookInput` | hooks/traits.rs |
| `HookOutput` | hooks/traits.rs |
| `HookContext` | hooks/traits.rs |
| `Hook` trait | hooks/traits.rs |
| `FnHook` | hooks/traits.rs |
| `HookManager` | hooks/manager.rs |
| `CommandHook` | hooks/command.rs |
| `HookRule` | hooks/rule.rs |
| `HookAction` | hooks/rule.rs |

## Shared Types: HookRule & HookAction

`HookRule` and `HookAction` are shared types used by plugins, skills, and subagents to define lifecycle hooks in frontmatter.

```rust
pub struct HookRule {
    pub matcher: Option<String>,   // Tool pattern (e.g., "Write|Edit")
    pub hooks: Vec<HookAction>,
}

pub struct HookAction {
    pub hook_type: String,         // Currently only "command"
    pub command: Option<String>,
    pub timeout: Option<u64>,
}
```

### Usage in frontmatter (skills/subagents)

```yaml
hooks:
  PreToolUse:
    - matcher: "Write|Edit"
      hooks:
        - type: command
          command: scripts/format.sh
          timeout: 10
```

### Official hooks.json format (plugins)

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          { "type": "command", "command": "fmt.sh" }
        ]
      }
    ]
  }
}
```
