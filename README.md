# claude-agent-rs

**Claude API를 위한 프로덕션 레디 Rust SDK**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://docs.rs/claude-agent/badge.svg)](https://docs.rs/claude-agent)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

---

## 왜 claude-agent-rs인가?

| | CLI 기반 | claude-agent-rs |
|---|:---:|:---:|
| **메모리** | 120GB+ 누수 | ~50MB 안정 |
| **시작 시간** | ~2초 | ~100ms |
| **의존성** | Node.js 필요 | 순수 Rust |
| **스트리밍** | stdout 파싱 | 네이티브 async |

---

## 설치

```toml
[dependencies]
claude-agent = "0.1"
tokio = { version = "1", features = ["full"] }
```

---

## Quick Start

### 간단한 쿼리

```rust
use claude_agent::query;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let response = query("Rust의 장점을 설명해줘").await?;
    println!("{response}");
    Ok(())
}
```

### 스트리밍

```rust
use claude_agent::stream;
use futures::StreamExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut s = stream("양자 컴퓨팅이란?").await?;
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

    let result = agent.execute("main.rs의 버그를 수정해줘").await?;
    println!("{}", result.text());
    Ok(())
}
```

---

## Cloud Providers

AWS Bedrock, Google Vertex AI, Azure Foundry를 네이티브 지원합니다.

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

환경변수로도 설정 가능:
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

## 내장 Tools

| Tool | 설명 |
|------|------|
| `Read` | 파일 읽기 |
| `Write` | 파일 쓰기 |
| `Edit` | 파일 편집 (문자열 치환) |
| `Bash` | 셸 명령 실행 |
| `Glob` | 패턴으로 파일 찾기 |
| `Grep` | 정규식으로 내용 검색 |
| `Task` | 서브 에이전트 실행 |
| `Skill` | 스킬 실행 |

---

## 커스텀 Tool 추가

```rust
use claude_agent::{Tool, ToolResult};
use async_trait::async_trait;

struct MyTool;

#[async_trait]
impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }
    fn description(&self) -> &str { "도구 설명" }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": { "input": { "type": "string" } },
            "required": ["input"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        ToolResult::success("결과")
    }
}

let agent = Agent::builder().tool(MyTool).build()?;
```

---

## License

MIT 또는 Apache-2.0 (선택)
