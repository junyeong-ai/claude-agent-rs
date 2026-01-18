# claude-agent-rs

**Production-Ready Rust SDK for Claude API**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://img.shields.io/docsrs/claude-agent)](https://docs.rs/claude-agent)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/crates/l/claude-agent.svg)](LICENSE)
[![DeepWiki](https://img.shields.io/badge/DeepWiki-junyeong--ai%2Fclaude--agent--rs-blue.svg?logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACwAAAAyCAYAAAAnWDnqAAAAAXNSR0IArs4c6QAAA05JREFUaEPtmUtyEzEQhtWTQyQLHNak2AB7ZnyXZMEjXMGeK/AIi+QuHrMnbChYY7MIh8g01fJoopFb0uhhEqqcbWTp06/uv1saEDv4O3n3dV60RfP947Mm9/SQc0ICFQgzfc4CYZoTPAswgSJCCUJUnAAoRHOAUOcATwbmVLWdGoH//PB8mnKqScAhsD0kYP3j/Yt5LPQe2KvcXmGvRHcDnpxfL2zOYJ1mFwrryWTz0advv1Ut4CJgf5uhDuDj5eUcAUoahrdY/56ebRWeraTjMt/00Sh3UDtjgHtQNHwcRGOC98BJEAEymycmYcWwOprTgcB6VZ5JK5TAJ+fXGLBm3FDAmn6oPPjR4rKCAoJCal2eAiQp2x0vxTPB3ALO2CRkwmDy5WohzBDwSEFKRwPbknEggCPB/imwrycgxX2NzoMCHhPkDwqYMr9tRcP5qNrMZHkVnOjRMWwLCcr8ohBVb1OMjxLwGCvjTikrsBOiA6fNyCrm8V1rP93iVPpwaE+gO0SsWmPiXB+jikdf6SizrT5qKasx5j8ABbHpFTx+vFXp9EnYQmLx02h1QTTrl6eDqxLnGjporxl3NL3agEvXdT0WmEost648sQOYAeJS9Q7bfUVoMGnjo4AZdUMQku50McDcMWcBPvr0SzbTAFDfvJqwLzgxwATnCgnp4wDl6Aa+Ax283gghmj+vj7feE2KBBRMW3FzOpLOADl0Isb5587h/U4gGvkt5v60Z1VLG8BhYjbzRwyQZemwAd6cCR5/XFWLYZRIMpX39AR0tjaGGiGzLVyhse5C9RKC6ai42ppWPKiBagOvaYk8lO7DajerabOZP46Lby5wKjw1HCRx7p9sVMOWGzb/vA1hwiWc6jm3MvQDTogQkiqIhJV0nBQBTU+3okKCFDy9WwferkHjtxib7t3xIUQtHxnIwtx4mpg26/HfwVNVDb4oI9RHmx5WGelRVlrtiw43zboCLaxv46AZeB3IlTkwouebTr1y2NjSpHz68WNFjHvupy3q8TFn3Hos2IAk4Ju5dCo8B3wP7VPr/FGaKiG+T+v+TQqIrOqMTL1VdWV1DdmcbO8KXBz6esmYWYKPwDL5b5FA1a0hwapHiom0r/cKaoqr+27/XcrS5UwSMbQAAAABJRU5ErkJggg==)](https://deepwiki.com/junyeong-ai/claude-agent-rs)

English | [한국어](README.ko.md)

---

## Why claude-agent-rs?

| Feature | claude-agent-rs | Other SDKs |
|---------|:---:|:---:|
| **Pure Rust, No Runtime Dependencies** | Native | Node.js/Python required |
| **Claude Code CLI Auth Reuse** | OAuth token sharing | Manual API key setup |
| **Automatic Prompt Caching** | System + Message caching | Manual implementation |
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
        .from_claude_code("./my-project").await?  // Auth + working_dir
        .tools(ToolAccess::all())                 // 12 built-in tools
        .with_web_search()                        // + Server tools
        .build()
        .await?;

    let stream = agent.execute_stream("Find and fix the bug in main.rs").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolComplete { name, .. } => eprintln!("\n[{name}]"),
            AgentEvent::Complete(result) => {
                eprintln!("\nTokens: {}", result.total_tokens());
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
    .from_claude_code(".").await?  // Uses ~/.claude/credentials.json
    .build()
    .await?
```

### API Key

```rust
use claude_agent::Auth;

Agent::builder()
    .auth(Auth::api_key("sk-ant-...")).await?
    .build()
    .await?
```

### Cloud Providers

```rust
use claude_agent::Auth;

// AWS Bedrock
Agent::builder()
    .auth(Auth::bedrock("us-east-1")).await?
    .build().await?

// Google Vertex AI
Agent::builder()
    .auth(Auth::vertex("project-id", "us-central1")).await?
    .build().await?

// Azure AI Foundry
Agent::builder()
    .auth(Auth::foundry("resource-name")).await?
    .build().await?
```

### Mixed: Any Auth + Claude Code Resources

```rust
// Use Bedrock auth with .claude/ project resources
Agent::builder()
    .auth(Auth::bedrock("us-east-1")).await?
    .working_dir("./my-project")
    .with_project_resources()  // Load .claude/ without OAuth
    .build().await?
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

### Prompt Caching

Automatic caching based on Anthropic best practices:

- **System prompt caching**: Static context cached with 1-hour TTL
- **Message history caching**: Last user turn cached with 5-minute TTL

```rust
use claude_agent::{CacheConfig, CacheStrategy};

Agent::builder()
    .cache(CacheConfig::default())        // Full caching (recommended)
    // .cache(CacheConfig::system_only()) // System prompt only (1h TTL)
    // .cache(CacheConfig::messages_only()) // Messages only (5m TTL)
    // .cache(CacheConfig::disabled())    // No caching
```

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
| JSONL | `jsonl` | CLI-compatible |
| PostgreSQL | `postgres` | Production |
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
use claude_agent::{McpManager, McpServerConfig};
use std::collections::HashMap;

let mut mcp = McpManager::new();
mcp.add_server("filesystem", McpServerConfig::Stdio {
    command: "npx".into(),
    args: vec!["-y".into(), "@anthropic-ai/mcp-server-filesystem".into()],
    env: HashMap::new(),
}).await?;

Agent::builder().mcp_manager(mcp).build().await?
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
| [Session](docs/session.md) | Persistence and prompt caching |
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
| `jsonl` | JSONL persistence (CLI-compatible) |
| `postgres` | PostgreSQL persistence |
| `redis-backend` | Redis persistence |
| `otel` | OpenTelemetry |
| `full` | All features |

---

## Examples

```bash
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
cargo test                    # 1000+ tests
cargo test -- --ignored       # + live API tests
cargo clippy --all-features   # Lint
```

---

## License

MIT
