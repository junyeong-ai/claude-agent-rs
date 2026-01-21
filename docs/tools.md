# Built-in Tools

claude-agent-rs includes 12 built-in tools + 3 server tools.

## Overview

| Category | Tools | Description |
|----------|-------|-------------|
| File | Read, Write, Edit, Glob, Grep | File system operations |
| Execution | Bash, KillShell | Shell command execution |
| Agent | Task, TaskOutput, TodoWrite, Skill | Agent orchestration |
| Planning | Plan | Structured planning workflow |

### Server Tools (Anthropic API)

| Tool | Description | Enable |
|------|-------------|--------|
| WebFetch | Fetch URL content | `.with_web_fetch()` |
| WebSearch | Web search | `.with_web_search()` |
| ToolSearch | Search tools (regex/BM25) | `.with_tool_search()` |

## File Tools

### Read

Read file contents with support for various formats.

```rust
// Supports: text, images (PNG, JPG), PDF, Jupyter notebooks (.ipynb)
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path to file |
| `offset` | number | No | Starting line number |
| `limit` | number | No | Maximum lines to read |

### Write

Create or overwrite files.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path |
| `content` | string | Yes | File content |

### Edit

String replacement-based file editing.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | Yes | Absolute path |
| `old_string` | string | Yes | Text to find |
| `new_string` | string | Yes | Replacement text |
| `replace_all` | boolean | No | Replace all occurrences |

### Glob

Pattern-based file search.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | Yes | Glob pattern (e.g., `**/*.rs`) |
| `path` | string | No | Base directory |

### Grep

Content search with regex support (ripgrep-based).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | Yes | Regex pattern |
| `path` | string | No | Search directory |
| `glob` | string | No | File filter pattern |
| `type` | string | No | File type (e.g., `rs`, `py`) |
| `output_mode` | string | No | `files_with_matches`, `content`, `count` |

## Execution Tools

### Bash

Execute shell commands with timeout and background support.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `command` | string | Yes | Shell command |
| `timeout` | number | No | Timeout in ms (max 600000) |
| `run_in_background` | boolean | No | Run asynchronously |
| `dangerouslyDisableSandbox` | boolean | No | Bypass OS sandbox (use with caution) |

**Security**: Commands are analyzed via AST (tree-sitter) before execution. OS-level sandboxing (Landlock/Seatbelt) can be enabled.

### KillShell

Terminate background processes.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `shell_id` | string | Yes | Process ID to kill |

## Agent Tools

### Task

Spawn subagents for complex tasks.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `description` | string | Yes | Short task description |
| `prompt` | string | Yes | Detailed instructions |
| `subagent_type` | string | Yes | Agent type (e.g., `explore`, `plan`) |
| `model` | string | No | Model override |
| `run_in_background` | boolean | No | Run asynchronously |
| `resume` | string | No | Agent ID to resume |

### TaskOutput

Retrieve results from background tasks.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `task_id` | string | Yes | Task ID |
| `block` | boolean | No | Wait for completion (default: true) |
| `timeout` | number | No | Max wait time in ms |

### TodoWrite

Manage task lists for progress tracking.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `todos` | array | Yes | List of todo items |

Todo item structure:
```json
{
  "content": "Task description",
  "status": "pending|in_progress|completed",
  "activeForm": "Present tense action"
}
```

### Skill

Execute registered skills.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `skill` | string | Yes | Skill name |
| `args` | string | No | Arguments |

## Server Tools

Server tools are Anthropic API-provided tools that run server-side.

### WebFetch

Fetch and process URL content.

```rust
let agent = Agent::builder()
    .auth(Auth::from_env()).await?
    .with_web_fetch()
    .build().await?;
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `max_uses` | u32 | Maximum uses per request |
| `allowed_domains` | array | Domain whitelist |
| `blocked_domains` | array | Domain blacklist |
| `max_content_tokens` | u32 | Max content tokens |

### WebSearch

Web search via Anthropic's search API.

```rust
let agent = Agent::builder()
    .auth(Auth::from_env()).await?
    .with_web_search()
    .build().await?;
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `max_uses` | u32 | Maximum uses per request |
| `allowed_domains` | array | Domain whitelist |
| `blocked_domains` | array | Domain blacklist |
| `user_location` | object | User location for localized results |

### ToolSearch

Search available tools using regex or BM25 algorithms.

```rust
use claude_agent::ToolSearchTool;

let agent = Agent::builder()
    .auth(Auth::from_env()).await?
    .with_tool_search(ToolSearchTool::regex())  // or ToolSearchTool::bm25()
    .build().await?;
```

| Variant | Type ID | Description |
|---------|---------|-------------|
| `regex()` | `tool_search_tool_regex_20251119` | Regex-based tool search |
| `bm25()` | `tool_search_tool_bm25_20251119` | BM25-based tool search |

## Tool Access Control

```rust
// All tools
Agent::builder().tools(ToolAccess::all())

// Specific tools only
Agent::builder().tools(ToolAccess::only(["Read", "Grep", "Glob"]))

// Exclude specific tools
Agent::builder().tools(ToolAccess::except(["Bash", "Write"]))
```

## Custom Tools

Implement the `Tool` trait:

```rust
use claude_agent::{Tool, ToolOutput};
use async_trait::async_trait;

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "My custom tool" }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string" }
            },
            "required": ["input"]
        })
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolOutput {
        ToolOutput::success("Result")
    }
}
```

Or use `SchemaTool` for automatic schema generation:

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize)]
struct MyInput {
    value: String,
}

struct MySchemaTool;

#[async_trait]
impl SchemaTool for MySchemaTool {
    type Input = MyInput;
    const NAME: &'static str = "my_schema_tool";
    const DESCRIPTION: &'static str = "Schema tool with auto-generated input schema";

    async fn handle(&self, input: Self::Input, ctx: &ToolContext) -> ToolResult {
        ToolResult::success(format!("Got: {}", input.value))
    }
}
```

## Tool Registration

```rust
let mut registry = ToolRegistry::new();

// Dynamic registration
registry.register_dynamic(Arc::new(MyTool))?;

// Replace existing
registry.register_or_replace(Arc::new(MyTool));

// Remove
registry.unregister("my_tool");
```
