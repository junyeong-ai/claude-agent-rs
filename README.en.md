# claude-agent-rs

**Production-Ready Rust SDK for Claude API**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://docs.rs/claude-agent/badge.svg)](https://docs.rs/claude-agent)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

---

## Why claude-agent-rs?

| | CLI-based | claude-agent-rs |
|---|:---:|:---:|
| **Memory** | 120GB+ leak | ~50MB stable |
| **Startup** | ~2s | ~100ms |
| **Dependency** | Node.js required | Pure Rust |
| **Streaming** | stdout parsing | Native async |

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
    let response = query("Explain the benefits of Rust").await?;
    println!("{response}");
    Ok(())
}
```

### Streaming

```rust
use claude_agent::stream;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut s = stream("What is quantum computing?").await?;
    while let Some(chunk) = s.next().await {
        print!("{}", chunk?);
    }
    Ok(())
}
```

### Agent + Tools

```rust
use claude_agent::{Agent, ToolAccess};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let agent = Agent::builder()
        .tools(ToolAccess::all())  // Read, Write, Bash, Glob, Grep...
        .working_dir("./my-project")
        .build()?;

    let result = agent.execute("Fix the bug in main.rs").await?;
    println!("{}", result.text());
    Ok(())
}
```

---

## Cloud Providers

Native support for AWS Bedrock, Google Vertex AI, and Azure Foundry.

```rust
// AWS Bedrock
let client = Client::builder()
    .bedrock("us-east-1")
    .model("anthropic.claude-3-5-sonnet-20241022-v2:0")
    .build()?;

// Google Vertex AI
let client = Client::builder()
    .vertex("my-project", "us-central1")
    .build()?;

// Azure AI Foundry
let client = Client::builder()
    .foundry("my-resource", "claude-sonnet")
    .build()?;
```

Environment variables also supported:
```bash
# Bedrock
export CLAUDE_CODE_USE_BEDROCK=1
export AWS_REGION=us-east-1

# Vertex AI
export CLAUDE_CODE_USE_VERTEX=1
export ANTHROPIC_VERTEX_PROJECT_ID=my-project

# Foundry
export CLAUDE_CODE_USE_FOUNDRY=1
export AZURE_RESOURCE_NAME=my-resource
```

---

## Built-in Tools

| Tool | Description |
|------|-------------|
| `Read` | Read files |
| `Write` | Write files |
| `Edit` | Edit files (string replacement) |
| `Bash` | Execute shell commands |
| `Glob` | Find files by pattern |
| `Grep` | Search content with regex |
| `Task` | Run sub-agents |
| `Skill` | Execute skills |

---

## Custom Tools

```rust
use claude_agent::{Tool, ToolResult};
use async_trait::async_trait;

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "Tool description" }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "input": { "type": "string" } },
            "required": ["input"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        ToolResult::success("result")
    }
}

let agent = Agent::builder().tool(MyTool).build()?;
```

---

## License

MIT or Apache-2.0 (at your option)
