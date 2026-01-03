# claude-agent-rs

**프로덕션 레디 Claude API Rust SDK**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://img.shields.io/docsrs/claude-agent)](https://docs.rs/claude-agent)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/crates/l/claude-agent.svg)](LICENSE)

[English](README.md) | 한국어

---

## 왜 claude-agent-rs인가?

| 기능 | claude-agent-rs | 기타 SDK |
|------|:---:|:---:|
| **순수 Rust, 런타임 의존성 없음** | 네이티브 | Node.js/Python 필요 |
| **Claude Code CLI 인증 재사용** | OAuth 토큰 공유 | 수동 API 키 설정 |
| **자동 Prompt Caching** | 최대 90% 비용 절감 | 수동 구현 |
| **TOCTOU-Safe 파일 연산** | `openat()` + `O_NOFOLLOW` | 표준 파일 I/O |
| **멀티 클라우드 지원** | Bedrock, Vertex, Foundry | 제한적 또는 없음 |
| **OS 레벨 샌드박싱** | Landlock, Seatbelt | 없음 |
| **1000+ 테스트** | 프로덕션 검증됨 | 다양함 |

---

## 빠른 시작

### 설치

```toml
[dependencies]
claude-agent = "0.2"
tokio = { version = "1", features = ["full"] }
```

### 간단한 쿼리

```rust
use claude_agent::query;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let response = query("Rust의 장점을 설명해줘").await?;
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
async fn main() -> claude_agent::Result<()> {
    let stream = stream("양자 컴퓨팅이란?").await?;
    let mut stream = pin!(stream);

    while let Some(chunk) = stream.next().await {
        print!("{}", chunk?);
    }
    Ok(())
}
```

### 도구를 포함한 에이전트

```rust
use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let agent = Agent::builder()
        .from_claude_code()              // Claude Code CLI OAuth 재사용
        .tools(ToolAccess::all())        // 12개 내장 도구
        .with_web_search()               // + 서버 도구
        .working_dir("./my-project")
        .build()
        .await?;

    let stream = agent.execute_stream("main.rs의 버그를 찾아서 수정해줘").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolStart { name, .. } => eprintln!("\n[{name}]"),
            AgentEvent::Complete(result) => {
                eprintln!("\n토큰: {} | 비용: ${:.4}",
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

## 인증

### Claude Code CLI (권장)

```rust
Agent::builder()
    .from_claude_code()  // ~/.claude/credentials.json 사용
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

### 클라우드 공급자

```rust
// AWS Bedrock
Agent::builder().bedrock("us-east-1").build().await?

// Google Vertex AI
Agent::builder().vertex("project-id", "us-central1").build().await?

// Azure AI Foundry
Agent::builder().foundry("resource-name").build().await?
```

자세한 내용: [인증 가이드](docs/authentication.md) | [클라우드 공급자](docs/cloud-providers.md)

---

## 도구

### 12개 내장 도구

| 카테고리 | 도구 |
|----------|------|
| **파일** | Read, Write, Edit, Glob, Grep |
| **셸** | Bash, KillShell |
| **에이전트** | Task, TaskOutput, TodoWrite, Skill |
| **계획** | Plan |

### 2개 서버 도구 (Anthropic API 전용)

| 도구 | 설명 |
|------|------|
| **WebFetch** | URL 콘텐츠 가져오기 및 처리 |
| **WebSearch** | 인용과 함께 웹 검색 |

```rust
Agent::builder()
    .with_web_fetch()
    .with_web_search()
```

### 도구 접근 제어

```rust
ToolAccess::all()                           // 12개 전체 도구
ToolAccess::only(["Read", "Grep", "Glob"])  // 특정 도구
ToolAccess::except(["Bash", "Write"])       // 도구 제외
```

자세한 내용: [도구 가이드](docs/tools.md)

---

## 핵심 기능

### 3개 내장 서브에이전트

| 타입 | 모델 | 용도 |
|------|------|------|
| `explore` | Haiku | 빠른 코드베이스 검색 |
| `plan` | Primary | 구현 계획 수립 |
| `general` | Primary | 복잡한 다단계 작업 |

자세한 내용: [서브에이전트 가이드](docs/subagents.md)

### 스킬 시스템

마크다운으로 재사용 가능한 스킬 정의:

`.claude/skills/deploy.md`:
```markdown
---
description: 프로덕션 배포
allowed-tools: [Bash, Read]
---
$ARGUMENTS 환경에 배포합니다.
```

자세한 내용: [스킬 가이드](docs/skills.md)

### 메모리 시스템

프로젝트 컨텍스트를 위해 `CLAUDE.md` 자동 로드:

```markdown
# 프로젝트 가이드
@import ./docs/architecture.md

## 규칙
- Rust 2024 Edition 사용
```

자세한 내용: [메모리 시스템 가이드](docs/memory-system.md)

### 세션 영속성

| 백엔드 | 기능 | 용도 |
|--------|------|------|
| Memory | (기본) | 개발 |
| PostgreSQL | `postgres` | 프로덕션 (7개 테이블) |
| Redis | `redis-backend` | 고처리량 |

자세한 내용: [세션 가이드](docs/session.md)

### 훅 시스템

실행 제어를 위한 10개 라이프사이클 이벤트:

| 차단 가능 | 차단 불가 |
|-----------|-----------|
| PreToolUse | PostToolUse, PostToolUseFailure |
| UserPromptSubmit | Stop, SubagentStart, SubagentStop |
| | PreCompact, SessionStart, SessionEnd |

자세한 내용: [훅 가이드](docs/hooks.md)

### MCP 통합

```rust
let mut mcp = McpManager::new();
mcp.add_server("filesystem", McpServerConfig::Stdio {
    command: "npx".into(),
    args: vec!["-y", "@anthropic-ai/mcp-server-filesystem"],
    env: HashMap::new(),
}).await?;

Agent::builder().mcp(mcp).build().await?
```

자세한 내용: [MCP 가이드](docs/mcp.md)

---

## 보안

| 기능 | 설명 |
|------|------|
| **OS 샌드박스** | Landlock (Linux 5.13+), Seatbelt (macOS) |
| **TOCTOU-Safe** | `openat()` + `O_NOFOLLOW` 원자적 연산 |
| **Bash AST** | tree-sitter 기반 위험 명령 탐지 |
| **리소스 제한** | `setrlimit()` 프로세스 격리 |
| **네트워크 필터** | 도메인 화이트리스트/블랙리스트 |

자세한 내용: [보안 가이드](docs/security.md) | [샌드박스 가이드](docs/sandbox.md)

---

## 문서

| 문서 | 설명 |
|------|------|
| [아키텍처](docs/architecture.md) | 시스템 구조 및 데이터 흐름 |
| [인증](docs/authentication.md) | OAuth, API Key, 클라우드 연동 |
| [도구](docs/tools.md) | 12개 내장 + 2개 서버 도구 |
| [스킬](docs/skills.md) | 슬래시 명령 및 스킬 정의 |
| [서브에이전트](docs/subagents.md) | 서브에이전트 생성 및 관리 |
| [메모리](docs/memory-system.md) | CLAUDE.md 및 @import |
| [훅](docs/hooks.md) | 10개 라이프사이클 이벤트 |
| [MCP](docs/mcp.md) | 외부 MCP 서버 |
| [세션](docs/session.md) | 영속성 및 압축 |
| [권한](docs/permissions.md) | 권한 모드 및 정책 |
| [보안](docs/security.md) | TOCTOU-safe 연산 |
| [샌드박스](docs/sandbox.md) | Landlock 및 Seatbelt |
| [예산](docs/budget.md) | 토큰/비용 제한 |
| [관측성](docs/observability.md) | OpenTelemetry 연동 |
| [출력 스타일](docs/output-styles.md) | 응답 형식 |
| [클라우드](docs/cloud-providers.md) | Bedrock, Vertex, Foundry |

---

## 기능 플래그

```toml
[dependencies]
claude-agent = { version = "0.2", features = ["mcp", "postgres"] }
```

| 기능 | 설명 |
|------|------|
| `cli-integration` | Claude Code CLI 지원 (기본) |
| `mcp` | MCP 프로토콜 지원 |
| `multimedia` | PDF 읽기 지원 |
| `aws` | AWS Bedrock |
| `gcp` | Google Vertex AI |
| `azure` | Azure AI Foundry |
| `postgres` | PostgreSQL 영속성 |
| `redis-backend` | Redis 영속성 |
| `otel` | OpenTelemetry |
| `full` | 모든 기능 |

---

## 예제

```bash
cargo run --example sdk_core_test      # 코어 SDK
cargo run --example advanced_test      # 스킬, 서브에이전트, 훅
cargo run --example all_tools_test     # 12개 전체 도구
cargo run --example server_tools       # WebFetch, WebSearch
```

---

## 환경 변수

| 변수 | 설명 |
|------|------|
| `ANTHROPIC_API_KEY` | API 키 |
| `ANTHROPIC_MODEL` | 기본 모델 |
| `CLAUDE_CODE_USE_BEDROCK` | Bedrock 활성화 |
| `CLAUDE_CODE_USE_VERTEX` | Vertex AI 활성화 |
| `CLAUDE_CODE_USE_FOUNDRY` | Foundry 활성화 |

---

## 테스트

```bash
cargo test                    # 1061개 테스트
cargo test -- --ignored       # + 라이브 API 테스트
cargo clippy --all-features   # 린트
```

---

## 라이선스

MIT
