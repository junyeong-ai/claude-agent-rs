# claude-agent-rs

**Production-Ready Rust SDK for Anthropic Claude API**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://docs.rs/claude-agent/badge.svg)](https://docs.rs/claude-agent)
[![CI](https://github.com/anthropics/claude-agent-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/anthropics/claude-agent-rs/actions/workflows/ci.yml)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

A pure Rust SDK that calls the Claude API directly without CLI subprocess dependencies. Memory-efficient, native async/await support, with built-in AWS Bedrock, Google Vertex AI, and Azure Foundry integration.

---

## Features

| Feature | Description |
|---------|-------------|
| **Pure Rust** | No Node.js dependency, ~50MB memory |
| **Native Streaming** | Real-time responses via `futures::Stream` |
| **Multi-Cloud** | Anthropic, Bedrock, Vertex AI, Foundry |
| **11 Built-in Tools** | Read, Write, Edit, Bash, Glob, Grep, etc. |
| **Skills System** | YAML frontmatter-based reusable workflows |
| **Memory System** | CLAUDE.md, @import syntax, hierarchical loading |

---

## Installation

```toml
[dependencies]
claude-agent = "0.1"
tokio = { version = "1", features = ["full"] }
```

---

## Quick Start

### Simple Query

```rust
use claude_agent::query;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let response = query("What are the benefits of Rust?").await?;
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
async fn main() -> anyhow::Result<()> {
    let stream = stream("Explain quantum computing").await?;
    let mut stream = pin!(stream);

    while let Some(chunk) = stream.next().await {
        print!("{}", chunk?);
    }
    Ok(())
}
```

### Agent + Tools

```rust
use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = Agent::builder()
        .model("claude-sonnet-4-5")
        .tools(ToolAccess::all())
        .working_dir("./my-project")
        .max_iterations(10)
        .build()?;

    let stream = agent.execute_stream("Fix the bug in main.rs").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolStart { name, .. } => eprintln!("\n[Tool: {name}]"),
            AgentEvent::Complete(result) => {
                eprintln!("\nDone: {} tokens", result.total_tokens());
            }
            _ => {}
        }
    }
    Ok(())
}
```

---

## Cloud Providers

### AWS Bedrock

```rust
let client = Client::builder()
    .bedrock("us-east-1")
    .model("anthropic.claude-3-5-sonnet-20241022-v2:0")
    .build()?;
```

```bash
export CLAUDE_CODE_USE_BEDROCK=1
export AWS_REGION=us-east-1
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...
```

### Google Vertex AI

```rust
let client = Client::builder()
    .vertex("my-project", "us-central1")
    .build()?;
```

```bash
export CLAUDE_CODE_USE_VERTEX=1
export ANTHROPIC_VERTEX_PROJECT_ID=my-project
export CLOUD_ML_REGION=us-central1
```

### Azure AI Foundry

```rust
let client = Client::builder()
    .foundry("my-resource", "claude-sonnet")
    .build()?;
```

```bash
export CLAUDE_CODE_USE_FOUNDRY=1
export AZURE_RESOURCE_NAME=my-resource
export AZURE_API_KEY=...
```

---

## Built-in Tools

| Tool | Description |
|------|-------------|
| `Read` | Read files (offset/limit support) |
| `Write` | Create/overwrite files |
| `Edit` | String replacement-based editing |
| `Bash` | Execute shell commands (timeout, background) |
| `Glob` | Pattern-based file search |
| `Grep` | Regex content search |
| `TodoWrite` | Task list management |
| `WebFetch` | Fetch URL content |
| `WebSearch` | Web search |
| `NotebookEdit` | Jupyter notebook editing |
| `KillShell` | Terminate background processes |

### Tool Access Control

```rust
// All tools
Agent::builder().tools(ToolAccess::all())

// Specific tools only
Agent::builder().tools(ToolAccess::only(["Read", "Grep", "Glob"]))

// Exclude specific tools
Agent::builder().tools(ToolAccess::except(["Bash", "Write"]))
```

---

## Custom Tools

```rust
use claude_agent::{Tool, ToolResult};
use async_trait::async_trait;

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str { "get_weather" }
    fn description(&self) -> &str { "Get current weather" }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "City name" }
            },
            "required": ["city"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let city = input["city"].as_str().unwrap_or("Unknown");
        ToolResult::success(format!("{city}: Sunny, 72°F"))
    }
}

let agent = Agent::builder()
    .tool(WeatherTool)
    .build()?;
```

---

## Skills System

`.claude/commands/deploy.md`:

```yaml
---
description: Production deployment
allowed-tools:
  - Bash
  - Read
---

Deploy to the $1 environment.

1. Run tests
2. Build
3. Deploy to $1
```

```rust
let agent = Agent::builder()
    .skill(SkillDefinition::new(
        "deploy",
        "Production deployment",
        "Deployment process: $ARGUMENTS",
    ))
    .build()?;
```

---

## Memory System

Automatically loads `CLAUDE.md` files to provide context:

```markdown
# Project Guide

@import ./docs/architecture.md
@import ~/global-rules.md

## Coding Rules
- Use Rust 2021 Edition
- Document all pub functions
```

Loading priority: `~/.claude/CLAUDE.md` → Project root → Current directory

---

## Authentication

```rust
// Environment variables (default)
let client = Client::from_env()?;

// Direct API Key
let client = Client::builder()
    .api_key("sk-ant-...")
    .build()?;

// Claude CLI OAuth token
let client = Client::builder()
    .from_claude_cli()
    .build()?;

// Auto-resolve (env → CLI → Bedrock → Vertex → Foundry)
let client = Client::builder()
    .auto_resolve()
    .build()?;
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `ANTHROPIC_MODEL` | Default model (default: claude-sonnet-4-5) |
| `ANTHROPIC_BASE_URL` | API endpoint |
| `CLAUDE_CODE_USE_BEDROCK` | Enable Bedrock |
| `CLAUDE_CODE_USE_VERTEX` | Enable Vertex AI |
| `CLAUDE_CODE_USE_FOUNDRY` | Enable Foundry |

---

## Examples

- [`examples/simple_query.rs`](examples/simple_query.rs) - Basic query
- [`examples/streaming.rs`](examples/streaming.rs) - Streaming response
- [`examples/agent_loop.rs`](examples/agent_loop.rs) - Agent execution loop

```bash
cargo run --example simple_query
cargo run --example streaming
cargo run --example agent_loop
```

---

## License

MIT or Apache-2.0 (at your option)
