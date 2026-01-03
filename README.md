# claude-agent-rs

**Production-Ready Rust SDK for Claude API**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://img.shields.io/docsrs/claude-agent)](https://docs.rs/claude-agent)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/crates/l/claude-agent.svg)](LICENSE)

English | [한국어](README.ko.md)

---

## Why claude-agent-rs?

| Feature | claude-agent-rs | Other SDKs |
|---------|:---:|:---:|
| **Pure Rust, No Runtime Dependencies** | Native | Node.js/Python required |
| **Claude Code CLI Auth Reuse** | OAuth token sharing | Manual API key setup |
| **Automatic Prompt Caching** | Up to 90% cost savings | Manual implementation |
| **TOCTOU-Safe File Operations** | `openat()` + `O_NOFOLLOW` | Standard file I/O |
| **Multi-Cloud Support** | Bedrock, Vertex, Foundry | Limited or none |
| **OS-Level Sandboxing** | Landlock, Seatbelt | None |
| **1000+ Tests** | Production-proven | Varies |

---

## Quick Start

### Installation

```toml
[dependencies]
claude-agent = "0.2"
tokio = { version = "1", features = ["full"] }
```

### Simple Query

```rust
use claude_agent::query;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let response = query("Explain the benefits of Rust").await?;
    println!("{response}");
    Ok(())
}
```

### Streaming Response

```rust
use claude_agent::stream;
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let stream = stream("What is quantum computing?").await?;
    let mut stream = pin!(stream);

    while let Some(chunk) = stream.next().await {
        print!("{}", chunk?);
    }
    Ok(())
}
```

### Full Agent with Tools

```rust
use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let agent = Agent::builder()
        .from_claude_code()              // Reuse Claude Code CLI OAuth
        .tools(ToolAccess::all())        // 12 built-in tools
        .with_web_search()               // + Server tools
        .working_dir("./my-project")
        .build()
        .await?;

    let stream = agent.execute_stream("Find and fix the bug in main.rs").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolStart { name, .. } => eprintln!("\n[{name}]"),
            AgentEvent::Complete(result) => {
                eprintln!("\nTokens: {} | Cost: ${:.4}",
                    result.total_tokens(),
                    result.total_cost_usd());
            }
            _ => {}
        }
    }
    Ok(())
}
```

---

## Authentication

### Claude Code CLI (Recommended)

```rust
Agent::builder()
    .from_claude_code()  // Uses ~/.claude/credentials.json
    .build()
    .await?
```

### API Key

```rust
Agent::builder()
    .api_key("sk-ant-...")
    .build()
    .await?
```

### Cloud Providers

```rust
// AWS Bedrock
Agent::builder().bedrock("us-east-1").build().await?

// Google Vertex AI
Agent::builder().vertex("project-id", "us-central1").build().await?

// Azure AI Foundry
Agent::builder().foundry("resource-name").build().await?
```

See: [Authentication Guide](docs/authentication.md) | [Cloud Providers](docs/cloud-providers.md)

---

## Tools

### 12 Built-in Tools

| Category | Tools |
|----------|-------|
| **File** | Read, Write, Edit, Glob, Grep |
| **Shell** | Bash, KillShell |
| **Agent** | Task, TaskOutput, TodoWrite, Skill |
| **Planning** | Plan |

### 2 Server Tools (Anthropic API only)

| Tool | Description |
|------|-------------|
| **WebFetch** | Fetch and process URL content |
| **WebSearch** | Web search with citations |

```rust
Agent::builder()
    .with_web_fetch()
    .with_web_search()
```

### Tool Access Control

```rust
ToolAccess::all()                           // All 12 tools
ToolAccess::only(["Read", "Grep", "Glob"])  // Specific tools
ToolAccess::except(["Bash", "Write"])       // Exclude tools
```

See: [Tools Guide](docs/tools.md)

---

## Key Features

### 3 Built-in Subagents

| Type | Model | Purpose |
|------|-------|---------|
| `explore` | Haiku | Fast codebase search |
| `plan` | Primary | Implementation planning |
| `general` | Primary | Complex multi-step tasks |

See: [Subagents Guide](docs/subagents.md)

### Skills System

Define reusable skills via markdown:

`.claude/skills/deploy.md`:
```markdown
---
description: Production deployment
allowed-tools: [Bash, Read]
---
Deploy to $ARGUMENTS environment.
```

See: [Skills Guide](docs/skills.md)

### Memory System

Auto-loads `CLAUDE.md` for project context:

```markdown
# Project Guide
@import ./docs/architecture.md

## Rules
- Use Rust 2024 Edition
```

See: [Memory System Guide](docs/memory-system.md)

### Session Persistence

| Backend | Feature | Use Case |
|---------|---------|----------|
| Memory | (default) | Development |
| PostgreSQL | `postgres` | Production (7 tables) |
| Redis | `redis-backend` | High-throughput |

See: [Session Guide](docs/session.md)

### Hooks System

10 lifecycle events for execution control:

| Blockable | Non-Blockable |
|-----------|---------------|
| PreToolUse | PostToolUse, PostToolUseFailure |
| UserPromptSubmit | Stop, SubagentStart, SubagentStop |
| | PreCompact, SessionStart, SessionEnd |

See: [Hooks Guide](docs/hooks.md)

### MCP Integration

```rust
let mut mcp = McpManager::new();
mcp.add_server("filesystem", McpServerConfig::Stdio {
    command: "npx".into(),
    args: vec!["-y", "@anthropic-ai/mcp-server-filesystem"],
    env: HashMap::new(),
}).await?;

Agent::builder().mcp(mcp).build().await?
```

See: [MCP Guide](docs/mcp.md)

---

## Security

| Feature | Description |
|---------|-------------|
| **OS Sandbox** | Landlock (Linux 5.13+), Seatbelt (macOS) |
| **TOCTOU-Safe** | `openat()` + `O_NOFOLLOW` atomic operations |
| **Bash AST** | tree-sitter based dangerous command detection |
| **Resource Limits** | `setrlimit()` process isolation |
| **Network Filter** | Domain whitelist/blacklist |

See: [Security Guide](docs/security.md) | [Sandbox Guide](docs/sandbox.md)

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/architecture.md) | System structure and data flow |
| [Authentication](docs/authentication.md) | OAuth, API Key, cloud integration |
| [Tools](docs/tools.md) | 12 built-in + 2 server tools |
| [Skills](docs/skills.md) | Slash commands and skill definitions |
| [Subagents](docs/subagents.md) | Subagent spawning and management |
| [Memory](docs/memory-system.md) | CLAUDE.md and @import |
| [Hooks](docs/hooks.md) | 10 lifecycle events |
| [MCP](docs/mcp.md) | External MCP servers |
| [Session](docs/session.md) | Persistence and compaction |
| [Permissions](docs/permissions.md) | Permission modes and policies |
| [Security](docs/security.md) | TOCTOU-safe operations |
| [Sandbox](docs/sandbox.md) | Landlock and Seatbelt |
| [Budget](docs/budget.md) | Token/cost limits |
| [Observability](docs/observability.md) | OpenTelemetry integration |
| [Output Styles](docs/output-styles.md) | Response formatting |
| [Cloud Providers](docs/cloud-providers.md) | Bedrock, Vertex, Foundry |

---

## Feature Flags

```toml
[dependencies]
claude-agent = { version = "0.2", features = ["mcp", "postgres"] }
```

| Feature | Description |
|---------|-------------|
| `cli-integration` | Claude Code CLI support (default) |
| `mcp` | MCP protocol support |
| `multimedia` | PDF reading support |
| `aws` | AWS Bedrock |
| `gcp` | Google Vertex AI |
| `azure` | Azure AI Foundry |
| `postgres` | PostgreSQL persistence |
| `redis-backend` | Redis persistence |
| `otel` | OpenTelemetry |
| `full` | All features |

---

## Examples

```bash
cargo run --example sdk_core_test      # Core SDK
cargo run --example advanced_test      # Skills, subagents, hooks
cargo run --example all_tools_test     # All 12 tools
cargo run --example server_tools       # WebFetch, WebSearch
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | API key |
| `ANTHROPIC_MODEL` | Default model |
| `CLAUDE_CODE_USE_BEDROCK` | Enable Bedrock |
| `CLAUDE_CODE_USE_VERTEX` | Enable Vertex AI |
| `CLAUDE_CODE_USE_FOUNDRY` | Enable Foundry |

---

## Testing

```bash
cargo test                    # 1061 tests
cargo test -- --ignored       # + live API tests
cargo clippy --all-features   # Lint
```

---

## License

MIT
