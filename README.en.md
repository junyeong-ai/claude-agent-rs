# claude-agent-rs

**Production-Ready Rust SDK with Full Claude Code CLI Compatibility**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://img.shields.io/docsrs/claude-agent)](https://docs.rs/claude-agent)
[![Rust](https://img.shields.io/badge/rust-1.92%2B_2024-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/crates/l/claude-agent.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-516%20passing-brightgreen.svg)](https://github.com/junyeong-ai/claude-agent-rs)

English | [한국어](README.md)

A pure Rust SDK that seamlessly uses Claude Code CLI's OAuth tokens. Directly calls the Claude API without Node.js subprocess, with full support for Prompt Caching and Progressive Disclosure.

---

## Why claude-agent-rs?

| Feature | Description |
|---------|-------------|
| **Claude Code CLI Compatible** | Use existing auth with just `from_claude_cli()` |
| **Prompt Caching** | Automatic system prompt caching saves 90% token costs |
| **Pure Rust** | No Node.js dependency, ~50MB memory |
| **13 Built-in Tools** | Read, Write, Edit, Bash, Glob, Grep, Task, and more |
| **Progressive Disclosure** | On-demand skill/rule loading for context optimization |
| **Multi-Cloud** | Anthropic, AWS Bedrock, Google Vertex AI, Azure Foundry |

---

## Installation

```toml
[dependencies]
claude-agent = "0.1"
tokio = { version = "1", features = ["full"] }
```

---

## Quick Start

### 1. Simple Query

```rust
use claude_agent::query;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let response = query("What are the benefits of Rust?").await?;
    println!("{response}");
    Ok(())
}
```

### 2. Streaming Response

```rust
use claude_agent::stream;
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let stream = stream("Explain quantum computing").await?;
    let mut stream = pin!(stream);

    while let Some(chunk) = stream.next().await {
        print!("{}", chunk?);
    }
    Ok(())
}
```

### 3. Agent + Tools (Core Usage)

```rust
use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let agent = Agent::builder()
        .from_claude_cli()              // Use Claude Code CLI auth
        .tools(ToolAccess::all())       // Enable all 13 tools
        .working_dir("./my-project")
        .max_iterations(10)
        .build()
        .await?;

    let stream = agent.execute_stream("Fix the bug in main.rs").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolStart { name, .. } => eprintln!("\n🔧 [{name}]"),
            AgentEvent::ToolEnd { .. } => eprintln!(" ✓"),
            AgentEvent::Complete(result) => {
                eprintln!("\n✅ Done: {} tokens, {} tool calls",
                    result.total_tokens(), result.tool_calls);
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

Use your existing Claude Code CLI authentication:

```rust
let client = Client::builder()
    .from_claude_cli()
    .build()?;

// Same for Agent
let agent = Agent::builder()
    .from_claude_cli()
    .build()
    .await?;
```

**Prerequisite**: CLI must be authenticated (`claude --version`).

**Automatically Included**:
- OAuth Bearer token
- Prompt Caching (`cache_control: ephemeral`)
- Required beta flags (`claude-code-20250219`, `oauth-2025-04-20`)

### API Key

```rust
let client = Client::builder()
    .api_key("sk-ant-...")
    .build()?;
```

### Environment Variable

```rust
// Uses ANTHROPIC_API_KEY automatically
let client = Client::from_env()?;
```

### Cloud Providers

```rust
// AWS Bedrock
let client = Client::builder()
    .bedrock("us-east-1")
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

---

## 13 Built-in Tools

### File Tools
| Tool | Description |
|------|-------------|
| `Read` | Read files (supports images, PDF, Jupyter notebooks) |
| `Write` | Create/overwrite files |
| `Edit` | Precise string replacement-based editing |
| `Glob` | Pattern-based file search (`**/*.rs`) |
| `Grep` | Regex content search (ripgrep-based) |
| `NotebookEdit` | Jupyter notebook cell editing |

### Execution Tools
| Tool | Description |
|------|-------------|
| `Bash` | Execute shell commands (timeout, background support) |
| `KillShell` | Terminate background processes |

### Agent Tools
| Tool | Description |
|------|-------------|
| `Task` | Create and run sub-agents |
| `TaskOutput` | Retrieve background task results |
| `TodoWrite` | Task list management |
| `Skill` | Execute registered skills |
| `WebFetch` | Fetch URL content |

### Tool Access Control

```rust
// Enable all tools
Agent::builder().tools(ToolAccess::all())

// Allow specific tools only
Agent::builder().tools(ToolAccess::only(["Read", "Grep", "Glob"]))

// Exclude specific tools (for security)
Agent::builder().tools(ToolAccess::except(["Bash", "Write"]))
```

---

## Progressive Disclosure

Dynamically loads skills and rules when needed for efficient context window usage.

### Skills System

```rust
use claude_agent::{Agent, SkillDefinition, ToolAccess};

let agent = Agent::builder()
    .from_claude_cli()
    .skill(SkillDefinition::new(
        "deploy",
        "Production deployment",
        "Deployment process: $ARGUMENTS\n1. Run tests\n2. Build\n3. Deploy",
    ).with_trigger("deploy").with_trigger("release"))
    .tools(ToolAccess::only(["Skill", "Bash"]))
    .build()
    .await?;
```

### Trigger-Based Activation

Skills activate via explicit calls or trigger keywords:

```rust
// Explicit: /deploy production
// Trigger: "deploy to production" → "deploy" keyword detected → deploy skill activates
```

### Slash Commands

`.claude/commands/deploy.md`:

```yaml
---
description: Production deployment
allowed-tools:
  - Bash
  - Read
---

Deploy to the $ARGUMENTS environment.
```

---

## Prompt Caching

Automatically enabled when using Claude Code CLI authentication.

### How It Works

```
First request:  cache_creation_input_tokens (cache created)
Second request: cache_read_input_tokens (cache hit, 90% cost savings)
```

### Cache Statistics

```rust
use claude_agent::session::CacheStats;

let stats = CacheStats::default();
stats.update(1000, 0);  // cache_read: 1000, cache_creation: 0

println!("Cache hit rate: {:.1}%", stats.hit_rate() * 100.0);
println!("Tokens saved: {}", stats.tokens_saved());
```

---

## Memory System

Automatically loads `CLAUDE.md` files to provide project context.

```markdown
# Project Guide

@import ./docs/architecture.md
@import ~/global-rules.md

## Coding Rules
- Use Rust 2021 Edition
- Document all pub functions
```

**Loading Priority**: `~/.claude/CLAUDE.md` → Project root → Current directory

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
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `ANTHROPIC_MODEL` | Default model (default: `claude-sonnet-4-5`) |
| `CLAUDE_CODE_USE_BEDROCK` | Enable AWS Bedrock |
| `CLAUDE_CODE_USE_VERTEX` | Enable Google Vertex AI |
| `CLAUDE_CODE_USE_FOUNDRY` | Enable Azure Foundry |

---

## Examples

```bash
# Basic query
cargo run --example simple_query

# Streaming response
cargo run --example streaming

# Agent execution loop
cargo run --example agent_loop

# Full tool verification
cargo run --example comprehensive_test
```

---

## Testing

```bash
# Unit tests (no auth required)
cargo test

# Include CLI auth tests
cargo test -- --ignored

# Full verification
cargo run --example comprehensive_test
```

**Test Status**: 516 tests passing

---

## License

MIT or Apache-2.0 (at your option)
