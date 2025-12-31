# claude-agent-rs

**Anthropic Claude API를 위한 프로덕션 레디 Rust SDK**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://docs.rs/claude-agent/badge.svg)](https://docs.rs/claude-agent)
[![CI](https://github.com/anthropics/claude-agent-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/anthropics/claude-agent-rs/actions/workflows/ci.yml)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

CLI 서브프로세스 없이 Claude API를 직접 호출하는 순수 Rust SDK입니다. 메모리 효율적이고, 네이티브 async/await을 지원하며, AWS Bedrock, Google Vertex AI, Azure Foundry를 기본 지원합니다.

---

## 특징

| 기능 | 설명 |
|------|------|
| **순수 Rust** | Node.js 의존성 없음, ~50MB 메모리 |
| **네이티브 스트리밍** | `futures::Stream` 기반 실시간 응답 |
| **멀티 클라우드** | Anthropic, Bedrock, Vertex AI, Foundry |
| **14개 내장 도구** | Read, Write, Edit, Bash, Glob, Grep, Task 등 |
| **스킬 시스템** | YAML frontmatter 기반 재사용 워크플로우 |
| **메모리 시스템** | CLAUDE.md, @import 구문, 계층적 로딩 |

---

## 설치

```toml
[dependencies]
claude-agent = "0.1"
tokio = { version = "1", features = ["full"] }
```

---

## 빠른 시작

### 간단한 쿼리

```rust
use claude_agent::query;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let response = query("Rust의 장점은?").await?;
    println!("{response}");
    Ok(())
}
```

### 스트리밍 응답

```rust
use claude_agent::stream;
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let stream = stream("양자 컴퓨팅을 설명해줘").await?;
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

    let stream = agent.execute_stream("main.rs의 버그를 수정해줘").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolStart { name, .. } => eprintln!("\n[Tool: {name}]"),
            AgentEvent::Complete(result) => {
                eprintln!("\n완료: {} tokens", result.total_tokens());
            }
            _ => {}
        }
    }
    Ok(())
}
```

---

## 클라우드 공급자

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

## 내장 도구

### 파일
| 도구 | 설명 |
|------|------|
| `Read` | 파일 읽기 (이미지, PDF, Jupyter 노트북 포함) |
| `Write` | 파일 생성/덮어쓰기 |
| `Edit` | 문자열 치환 기반 편집 |
| `NotebookEdit` | Jupyter 노트북 셀 편집 |
| `Glob` | 패턴 기반 파일 검색 |
| `Grep` | 정규식 내용 검색 |

### 실행
| 도구 | 설명 |
|------|------|
| `Bash` | 셸 명령 실행 (타임아웃, 백그라운드) |
| `KillShell` | 백그라운드 프로세스 종료 |

### 웹
| 도구 | 설명 |
|------|------|
| `WebFetch` | URL 콘텐츠 가져오기 |
| `WebSearch` | 웹 검색 |

### 에이전트
| 도구 | 설명 |
|------|------|
| `Task` | 서브에이전트 실행 |
| `TaskOutput` | 백그라운드 작업 결과 조회 |
| `TodoWrite` | 작업 목록 관리 |
| `Skill` | 등록된 스킬 실행 |

### 도구 접근 제어

```rust
// 모든 도구
Agent::builder().tools(ToolAccess::all())

// 특정 도구만
Agent::builder().tools(ToolAccess::only(["Read", "Grep", "Glob"]))

// 특정 도구 제외
Agent::builder().tools(ToolAccess::except(["Bash", "Write"]))
```

---

## 커스텀 도구

```rust
use claude_agent::{Tool, ToolResult};
use async_trait::async_trait;

struct WeatherTool;

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str { "get_weather" }
    fn description(&self) -> &str { "현재 날씨 조회" }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "도시 이름" }
            },
            "required": ["city"]
        })
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let city = input["city"].as_str().unwrap_or("Unknown");
        ToolResult::success(format!("{city}: 맑음, 22°C"))
    }
}

let agent = Agent::builder()
    .tool(WeatherTool)
    .build()?;
```

---

## 스킬 시스템

`.claude/commands/deploy.md`:

```yaml
---
description: 프로덕션 배포
allowed-tools:
  - Bash
  - Read
---

$1 환경에 배포합니다.

1. 테스트 실행
2. 빌드
3. $1 환경에 배포
```

```rust
let agent = Agent::builder()
    .skill(SkillDefinition::new(
        "deploy",
        "프로덕션 배포",
        "배포 프로세스: $ARGUMENTS",
    ))
    .build()?;
```

---

## 메모리 시스템

`CLAUDE.md` 파일을 자동으로 로드하여 컨텍스트를 제공합니다:

```markdown
# 프로젝트 가이드

@import ./docs/architecture.md
@import ~/global-rules.md

## 코딩 규칙
- Rust 2021 Edition 사용
- 모든 pub 함수에 문서화 필수
```

로딩 우선순위: `~/.claude/CLAUDE.md` → 프로젝트 루트 → 현재 디렉토리

---

## 인증

```rust
// 환경변수 (기본)
let client = Client::from_env()?;

// API Key 직접 지정
let client = Client::builder()
    .api_key("sk-ant-...")
    .build()?;

// Claude CLI OAuth 토큰
let client = Client::builder()
    .from_claude_cli()
    .build()?;

// 자동 해석 (env → CLI → Bedrock → Vertex → Foundry)
let client = Client::builder()
    .auto_resolve()
    .build()?;
```

---

## 환경 변수

| 변수 | 설명 |
|------|------|
| `ANTHROPIC_API_KEY` | Anthropic API 키 |
| `ANTHROPIC_MODEL` | 기본 모델 (default: claude-sonnet-4-5) |
| `ANTHROPIC_BASE_URL` | API 엔드포인트 |
| `CLAUDE_CODE_USE_BEDROCK` | Bedrock 사용 |
| `CLAUDE_CODE_USE_VERTEX` | Vertex AI 사용 |
| `CLAUDE_CODE_USE_FOUNDRY` | Foundry 사용 |

---

## 예제

- [`examples/simple_query.rs`](examples/simple_query.rs) - 기본 쿼리
- [`examples/streaming.rs`](examples/streaming.rs) - 스트리밍 응답
- [`examples/agent_loop.rs`](examples/agent_loop.rs) - Agent 실행 루프

```bash
cargo run --example simple_query
cargo run --example streaming
cargo run --example agent_loop
```

---

## License

MIT 또는 Apache-2.0 (선택)
