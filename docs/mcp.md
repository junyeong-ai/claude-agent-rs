# MCP (Model Context Protocol) Integration

MCP enables Claude to connect to external servers that provide tools and resources.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    MCP Architecture                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌──────────────────────────────────────────────┐          │
│   │                 McpManager                    │          │
│   │    (Multiple server connections)             │          │
│   └─────────────┬────────────────────────────────┘          │
│                 │                                            │
│       ┌─────────┼─────────┐                                 │
│       ▼         ▼         ▼                                 │
│   ┌────────┐ ┌────────┐ ┌────────┐                          │
│   │McpClient│ │McpClient│ │McpClient│                        │
│   │(server1)│ │(server2)│ │(server3)│                        │
│   └────┬────┘ └────┬────┘ └────┬────┘                        │
│        │           │           │                             │
│        ▼           ▼           ▼                             │
│   ┌────────┐ ┌────────┐ ┌────────┐                          │
│   │ stdio  │ │  SSE   │ │ stdio  │   Transport              │
│   └────────┘ └────────┘ └────────┘                          │
│        │           │           │                             │
│        ▼           ▼           ▼                             │
│   [Server]    [Server]    [Server]                          │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Transport Types

Three transport protocols supported:

| Transport | Description | Use Case |
|-----------|-------------|----------|
| `Stdio` | stdin/stdout communication | Local servers |
| `Sse` | Server-Sent Events | Remote servers, streaming |

## Configuration

### McpServerConfig

```rust
pub enum McpServerConfig {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        cwd: Option<String>,            // Working directory
    },
    Sse {
        url: String,
        headers: HashMap<String, String>,
    },
}
```

### Settings File

In `~/.claude/settings.json` or `.claude/settings.json`:

```json
{
  "mcpServers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@anthropic-ai/mcp-server-filesystem", "/home/user"],
      "env": {
        "MCP_DEBUG": "true"
      }
    },
    "github": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@anthropic-ai/mcp-server-github"],
      "env": {
        "GITHUB_TOKEN": "${GITHUB_TOKEN}"
      },
      "cwd": "/path/to/project"
    },
    "remote-api": {
      "type": "sse",
      "url": "https://api.example.com/mcp",
      "headers": {
        "Authorization": "Bearer ${API_KEY}"
      }
    }
  }
}
```

## Core Types

### McpClient

Manages a single server connection.

```rust
let mut client = McpClient::new("filesystem", McpServerConfig::Stdio {
    command: "npx".to_string(),
    args: vec!["-y".into(), "@anthropic-ai/mcp-server-filesystem".into()],
    env: HashMap::new(),
    cwd: None,
});

// Connect
client.connect().await?;

// Check connection
assert!(client.is_connected());

// List tools
for tool in client.tools() {
    println!("Tool: {} - {}", tool.name, tool.description);
}

// Call tool
let result = client.call_tool("read_file", json!({
    "path": "/tmp/test.txt"
})).await?;

// Read resource
let contents = client.read_resource("file:///tmp/test.txt").await?;

// Close
client.close().await?;
```

### McpManager

Manages multiple server connections.

```rust
let manager = McpManager::new();

// Add servers
manager.add_server("filesystem", McpServerConfig::Stdio { ... }).await?;
manager.add_server("github", McpServerConfig::Stdio { ... }).await?;

// List all tools (qualified names)
for (qualified_name, tool) in manager.list_tools().await {
    // Format: mcp__servername_toolname
    println!("{}: {}", qualified_name, tool.description);
}

// Call tool by qualified name
let result = manager.call_tool("mcp__filesystem_read_file", json!({
    "path": "/tmp/test.txt"
})).await?;

// Read resource from specific server
let contents = manager.read_resource("filesystem", "file:///tmp/test.txt").await?;

// List servers
let servers = manager.list_servers().await;

// Remove server
manager.remove_server("filesystem").await?;

// Close all
manager.close_all().await?;
```

### Tool Naming Convention

MCP tools are exposed with qualified names:

```
mcp__{server_name}_{tool_name}

Examples:
- mcp__filesystem_read_file
- mcp__github_create_issue
- mcp__slack_send_message
```

Check if a tool is MCP:
```rust
if McpManager::is_mcp_tool("mcp__filesystem_read_file") {
    // Handle MCP tool
}
```

### McpToolDefinition

```rust
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema
}
```

### McpResourceDefinition

```rust
pub struct McpResourceDefinition {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}
```

### McpToolResult

```rust
pub struct McpToolResult {
    pub content: Vec<McpContent>,
    pub is_error: bool,
}
```

### McpContent

```rust
pub enum McpContent {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource { uri: String, text: Option<String>, blob: Option<String>, mime_type: Option<String> },
}
```

## ResourceManager

High-level resource operations.

```rust
let resource_manager = ResourceManager::new(Arc::new(manager));

// List all resources
let resources = resource_manager.list_all().await;

// List from specific server
let files = resource_manager.list_from_server("filesystem").await?;

// Read as text
let content = resource_manager.read_text("filesystem", "file:///tmp/test.txt").await?;

// Subscribe to changes (future)
let sub_id = resource_manager.subscribe("filesystem", "file:///tmp/*.txt");

// Find by pattern
let matches = resource_manager.find_by_pattern("file://*").await;
```

### ResourceQuery Builder

```rust
let results = ResourceQuery::new()
    .server("filesystem")
    .pattern("file://*")
    .mime_type("text/plain")
    .execute(&resource_manager)
    .await;
```

## Connection Status

```rust
pub enum McpConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Failed,
    NeedsAuth,
}
```

## Server State

```rust
pub struct McpServerState {
    pub name: String,
    pub config: McpServerConfig,
    pub status: McpConnectionStatus,
    pub server_info: Option<McpServerInfo>,
    pub tools: Vec<McpToolDefinition>,
    pub resources: Vec<McpResourceDefinition>,
}
```

## Error Handling

```rust
pub enum McpError {
    ConnectionFailed { message: String },
    Protocol { message: String },
    JsonRpc { code: i32, message: String },
    ToolError { message: String },
    VersionMismatch { supported: Vec<String>, requested: String },
    ServerNotFound { name: String },
    ToolNotFound { name: String },
    ResourceNotFound { uri: String },
    Io(std::io::Error),
    Json(serde_json::Error),
}
```

## Agent Integration

```rust
use claude_agent::{Agent, mcp::McpManager};

let mut mcp_manager = McpManager::new();
mcp_manager.add_server("filesystem", config).await?;

let agent = Agent::builder()
    .from_claude_code(".").await?
    .mcp_manager(mcp_manager)
    .build()
    .await?;

// MCP tools automatically available as mcp__servername_toolname
```

## Feature Flag

MCP support requires the `mcp` feature:

```toml
[dependencies]
claude-agent = { version = "0.2", features = ["mcp"] }
```

Without the feature, MCP methods return stub errors.

## Protocol

Uses [rmcp](https://github.com/anthropics/rmcp) crate for protocol implementation.

### Initialization

1. Spawn server process (stdio) or connect (SSE)
2. Send `initialize` request
3. Receive server info and capabilities
4. List available tools
5. List available resources (optional)

### Tool Call

```json
// Request
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "read_file",
    "arguments": { "path": "/tmp/test.txt" }
  }
}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "content": [
      { "type": "text", "text": "File contents..." }
    ]
  }
}
```

### Resource Read

```json
// Request
{
  "jsonrpc": "2.0",
  "method": "resources/read",
  "params": {
    "uri": "file:///tmp/test.txt"
  }
}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "contents": [
      { "uri": "file:///tmp/test.txt", "text": "...", "mimeType": "text/plain" }
    ]
  }
}
```

## File Locations

| Type | File |
|------|------|
| `McpServerConfig` | mcp/mod.rs |
| `McpConnectionStatus` | mcp/mod.rs |
| `McpServerInfo` | mcp/mod.rs |
| `McpToolDefinition` | mcp/mod.rs |
| `McpResourceDefinition` | mcp/mod.rs |
| `McpServerState` | mcp/mod.rs |
| `McpError` | mcp/mod.rs |
| `McpToolResult` | mcp/mod.rs |
| `McpContent` | mcp/mod.rs |
| `McpClient` | mcp/client.rs |
| `McpManager` | mcp/manager.rs |
| `ResourceManager` | mcp/resources.rs |
| `ResourceQuery` | mcp/resources.rs |
