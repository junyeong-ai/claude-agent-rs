# claude-agent-rs

**Claude Code CLI Compatible Rust SDK**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://img.shields.io/docsrs/claude-agent)](https://docs.rs/claude-agent)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/crates/l/claude-agent.svg)](LICENSE)

English | [한국어](README.ko.md)

---

## Why claude-agent-rs?

| | claude-agent-rs | Other SDKs |
|---|:---:|:---:|
| **No Node.js dependency** | O | X |
| **Reuse Claude Code CLI auth** | O | X |
| **Auto Prompt Caching** | O | Manual |
| **TOCTOU-Safe file ops** | O | X |
| **Multi-cloud** | Bedrock, Vertex, Foundry | Limited |

---

## Quick Start

### Installation

```toml
[dependencies]
claude-agent = "0.2"
tokio = { version = "1", features = ["full"] }
```

### One-shot Query

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

### Agent Workflow

```rust
use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let agent = Agent::builder()
        .from_claude_code()              // Use Claude Code CLI auth
        .tools(ToolAccess::all())        // 12 tools + 2 server tools
        .working_dir("./my-project")
        .max_iterations(10)
        .build()
        .await?;

    let stream = agent.execute_stream("Find and fix the bug in main.rs").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolStart { name, .. } => eprintln!("\n[{name}]"),
            AgentEvent::ToolEnd { .. } => eprintln!(" done"),
            AgentEvent::Complete(result) => {
                eprintln!("\nTotal: {} tokens", result.total_tokens());
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
let agent = Agent::builder()
    .from_claude_code()  // Automatically use OAuth token
    .build()
    .await?;
```

### API Key

```rust
let agent = Agent::builder()
    .api_key("sk-ant-...")
    .build()
    .await?;
```

### Cloud Providers

```rust
// AWS Bedrock
let agent = Agent::builder().bedrock("us-east-1").build().await?;

// Google Vertex AI
let agent = Agent::builder().vertex("my-project", "us-central1").build().await?;

// Azure AI Foundry
let agent = Agent::builder().foundry("my-resource", "claude-sonnet").build().await?;
```

See: [Authentication Guide](docs/authentication.md) | [Cloud Providers](docs/cloud-providers.md)

---

## Tools

### 12 Built-in Tools

| Category | Tools | Description |
|----------|-------|-------------|
| **File** | Read, Write, Edit, Glob, Grep | File system operations |
| **Execution** | Bash, KillShell | Shell command execution |
| **Agent** | Task, TaskOutput, TodoWrite, Skill | Agent orchestration |
| **Planning** | Plan | Structured planning workflow |

### 2 Server Tools (Anthropic API)

| Tool | Description | Enable |
|------|-------------|--------|
| **WebFetch** | Fetch URL content | `.with_web_fetch()` |
| **WebSearch** | Web search | `.with_web_search()` |

### Tool Access Control

```rust
ToolAccess::all()                           // All tools
ToolAccess::only(["Read", "Grep", "Glob"])  // Specific tools only
ToolAccess::except(["Bash", "Write"])       // Exclude specific tools
```

See: [Tools Guide](docs/tools.md)

---

## Key Features

### Prompt Caching

Automatic system prompt caching for up to 90% token cost savings.

```rust
// Auto-enabled with from_claude_code
// Or manual configuration
let agent = Agent::builder()
    .cache_static_context(true)
    .build()
    .await?;
```

### Skills System

`.claude/commands/deploy.md`:
```markdown
---
description: Production deployment
allowed-tools: [Bash, Read]
---
Deploy to $ARGUMENTS environment.
```

Programmatic registration:
```rust
let skill = SkillDefinition::new("deploy", "Production deployment", "Deployment process...")
    .with_trigger("deploy")
    .with_allowed_tools(["Bash", "Read"]);

let agent = Agent::builder()
    .skill(skill)
    .build()
    .await?;
```

See: [Skills Guide](docs/skills.md)

### Subagents

Specialized agents running in isolated contexts:

| Type | Purpose | Model |
|------|---------|-------|
| `explore` | Codebase exploration | Haiku |
| `plan` | Implementation planning | Primary |
| `general` | General complex tasks | Primary |

```json
{
    "subagent_type": "explore",
    "prompt": "Analyze auth module structure",
    "run_in_background": true
}
```

See: [Subagents Guide](docs/subagents.md)

### Memory System

Auto-loads `CLAUDE.md` files for project context:

```markdown
# Project Guide

@import ./docs/architecture.md

## Coding Rules
- Use Rust 2024 Edition
```

See: [Memory System Guide](docs/memory-system.md)

---

## Security

| Feature | Description |
|---------|-------------|
| **OS Sandbox** | Landlock (Linux), Seatbelt (macOS) |
| **TOCTOU-Safe** | `openat()` + `O_NOFOLLOW` file operations |
| **Bash AST Analysis** | tree-sitter based dangerous command detection |
| **Resource Limits** | `setrlimit()` based process isolation |

See: [Security Guide](docs/security.md) | [Sandbox Guide](docs/sandbox.md)

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/architecture.md) | Overall system structure |
| [Authentication](docs/authentication.md) | OAuth, API Key, cloud integration |
| [Tools](docs/tools.md) | 12 built-in tools + 2 server tools |
| [Skills](docs/skills.md) | Skills system and slash commands |
| [Subagents](docs/subagents.md) | Subagent system |
| [Memory](docs/memory-system.md) | CLAUDE.md, @import |
| [Hooks](docs/hooks.md) | 10 event types, Pre/Post hooks |
| [MCP](docs/mcp.md) | External MCP server integration |
| [Session](docs/session.md) | Prompt Caching, context compaction |
| [Permissions](docs/permissions.md) | Permission modes and policies |
| [Security](docs/security.md) | TOCTOU-safe, Bash AST |
| [Sandbox](docs/sandbox.md) | Landlock, Seatbelt |
| [Budget](docs/budget.md) | Cost limits, tenant management |
| [Observability](docs/observability.md) | OpenTelemetry, metrics |
| [Output Styles](docs/output-styles.md) | Response format customization |
| [Cloud Providers](docs/cloud-providers.md) | Bedrock, Vertex, Foundry |

---

## Examples

```bash
# Core SDK test
cargo run --example sdk_core_test

# Advanced features test
cargo run --example advanced_test

# All tools test
cargo run --example all_tools_test

# Server tools (WebFetch, WebSearch)
cargo run --example server_tools

# Files API
cargo run --example files_api
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | API key |
| `ANTHROPIC_MODEL` | Default model |
| `CLAUDE_CODE_USE_BEDROCK` | Enable AWS Bedrock |
| `CLAUDE_CODE_USE_VERTEX` | Enable Google Vertex AI |
| `CLAUDE_CODE_USE_FOUNDRY` | Enable Azure Foundry |

---

## Testing

```bash
cargo test                    # Unit tests
cargo test -- --ignored       # Include CLI auth tests
cargo clippy --all-features   # Lint check
```

---

## License

MIT or Apache-2.0 (at your option)
