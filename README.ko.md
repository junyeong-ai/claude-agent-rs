# claude-agent-rs

**Claude Code CLI 완벽 호환 Rust SDK**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://img.shields.io/docsrs/claude-agent)](https://docs.rs/claude-agent)
[![Rust](https://img.shields.io/badge/rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/crates/l/claude-agent.svg)](LICENSE)

[English](README.md) | 한국어

---

## 왜 claude-agent-rs인가?

| | claude-agent-rs | 기타 SDK |
|---|:---:|:---:|
| **Node.js 의존성 없음** | O | X |
| **Claude Code CLI 인증 재사용** | O | X |
| **Prompt Caching 자동 적용** | O | 수동 |
| **TOCTOU-Safe 파일 연산** | O | X |
| **멀티 클라우드** | Bedrock, Vertex, Foundry | 제한적 |

---

## 빠른 시작

### 설치

```toml
[dependencies]
claude-agent = "0.2"
tokio = { version = "1", features = ["full"] }
```

### 원샷 쿼리

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

### Agent 워크플로우

```rust
use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let agent = Agent::builder()
        .from_claude_code()              // Claude Code CLI 인증 사용
        .tools(ToolAccess::all())        // 12개 도구 + 2개 서버 도구
        .working_dir("./my-project")
        .max_iterations(10)
        .build()
        .await?;

    let stream = agent.execute_stream("main.rs의 버그를 찾아서 수정해줘").await?;
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

## 인증

### Claude Code CLI (권장)

```rust
let agent = Agent::builder()
    .from_claude_code()  // OAuth 토큰 자동 사용
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

### 클라우드 공급자

```rust
// AWS Bedrock
let agent = Agent::builder().bedrock("us-east-1").build().await?;

// Google Vertex AI
let agent = Agent::builder().vertex("my-project", "us-central1").build().await?;

// Azure AI Foundry
let agent = Agent::builder().foundry("my-resource", "claude-sonnet").build().await?;
```

자세한 내용: [인증 가이드](docs/authentication.md) | [클라우드 공급자](docs/cloud-providers.md)

---

## 도구

### 12개 내장 도구

| 카테고리 | 도구 | 설명 |
|----------|------|------|
| **파일** | Read, Write, Edit, Glob, Grep | 파일 시스템 작업 |
| **실행** | Bash, KillShell | 셸 명령 실행 |
| **에이전트** | Task, TaskOutput, TodoWrite, Skill | 에이전트 오케스트레이션 |
| **계획** | Plan | 구조화된 계획 워크플로우 |

### 2개 서버 도구 (Anthropic API)

| 도구 | 설명 | 활성화 |
|------|------|--------|
| **WebFetch** | URL 콘텐츠 가져오기 | `.with_web_fetch()` |
| **WebSearch** | 웹 검색 | `.with_web_search()` |

### 도구 접근 제어

```rust
ToolAccess::all()                           // 모든 도구
ToolAccess::only(["Read", "Grep", "Glob"])  // 특정 도구만
ToolAccess::except(["Bash", "Write"])       // 특정 도구 제외
```

자세한 내용: [도구 가이드](docs/tools.md)

---

## 핵심 기능

### Prompt Caching

시스템 프롬프트 자동 캐싱으로 토큰 비용 최대 90% 절감.

```rust
// 자동 활성화 (from_claude_code 사용 시)
// 또는 수동 설정
let agent = Agent::builder()
    .cache_static_context(true)
    .build()
    .await?;
```

### 스킬 시스템

`.claude/commands/deploy.md`:
```markdown
---
description: 프로덕션 배포
allowed-tools: [Bash, Read]
---
$ARGUMENTS 환경에 배포합니다.
```

프로그래매틱 등록:
```rust
let skill = SkillDefinition::new("deploy", "프로덕션 배포", "배포 프로세스...")
    .with_trigger("deploy")
    .with_allowed_tools(["Bash", "Read"]);

let agent = Agent::builder()
    .skill(skill)
    .build()
    .await?;
```

자세한 내용: [스킬 가이드](docs/skills.md)

### 서브에이전트

독립 컨텍스트에서 실행되는 전문 에이전트:

| 타입 | 용도 | 모델 |
|------|------|------|
| `explore` | 코드베이스 탐색 | Haiku |
| `plan` | 구현 계획 수립 | Primary |
| `general` | 범용 복잡 작업 | Primary |

```json
{
    "subagent_type": "explore",
    "prompt": "인증 모듈 구조 분석",
    "run_in_background": true
}
```

자세한 내용: [서브에이전트 가이드](docs/subagents.md)

### 메모리 시스템

`CLAUDE.md` 파일 자동 로드로 프로젝트 컨텍스트 제공:

```markdown
# 프로젝트 가이드

@import ./docs/architecture.md

## 코딩 규칙
- Rust 2024 Edition 사용
```

자세한 내용: [메모리 시스템 가이드](docs/memory-system.md)

---

## 보안

| 기능 | 설명 |
|------|------|
| **OS 샌드박스** | Landlock (Linux), Seatbelt (macOS) |
| **TOCTOU-Safe** | `openat()` + `O_NOFOLLOW` 파일 연산 |
| **Bash AST 분석** | tree-sitter 기반 위험 명령 탐지 |
| **리소스 제한** | `setrlimit()` 기반 프로세스 격리 |

자세한 내용: [보안 가이드](docs/security.md) | [샌드박스 가이드](docs/sandbox.md)

---

## 문서

| 문서 | 설명 |
|------|------|
| [아키텍처](docs/architecture.md) | 전체 시스템 구조 |
| [인증](docs/authentication.md) | OAuth, API Key, 클라우드 연동 |
| [도구](docs/tools.md) | 12개 내장 도구 + 2개 서버 도구 |
| [스킬](docs/skills.md) | 스킬 시스템 및 슬래시 명령 |
| [서브에이전트](docs/subagents.md) | 서브에이전트 시스템 |
| [메모리](docs/memory-system.md) | CLAUDE.md, @import |
| [훅](docs/hooks.md) | 10개 이벤트 타입, Pre/Post 훅 |
| [MCP](docs/mcp.md) | 외부 MCP 서버 연동 |
| [세션](docs/session.md) | Prompt Caching, 컨텍스트 압축 |
| [권한](docs/permissions.md) | 권한 모드 및 정책 |
| [보안](docs/security.md) | TOCTOU-safe, Bash AST |
| [샌드박스](docs/sandbox.md) | Landlock, Seatbelt |
| [예산](docs/budget.md) | 비용 제한, 테넌트 관리 |
| [관측성](docs/observability.md) | OpenTelemetry, 메트릭 |
| [출력 스타일](docs/output-styles.md) | 응답 형식 커스터마이징 |
| [클라우드](docs/cloud-providers.md) | Bedrock, Vertex, Foundry |

---

## 예제

```bash
# 코어 SDK 테스트
cargo run --example sdk_core_test

# 고급 기능 테스트
cargo run --example advanced_test

# 전체 도구 테스트
cargo run --example all_tools_test

# 서버 도구 (WebFetch, WebSearch)
cargo run --example server_tools

# Files API
cargo run --example files_api
```

---

## 환경 변수

| 변수 | 설명 |
|------|------|
| `ANTHROPIC_API_KEY` | API 키 |
| `ANTHROPIC_MODEL` | 기본 모델 |
| `CLAUDE_CODE_USE_BEDROCK` | AWS Bedrock 활성화 |
| `CLAUDE_CODE_USE_VERTEX` | Google Vertex AI 활성화 |
| `CLAUDE_CODE_USE_FOUNDRY` | Azure Foundry 활성화 |

---

## 테스트

```bash
cargo test                    # 단위 테스트
cargo test -- --ignored       # CLI 인증 테스트 포함
cargo clippy --all-features   # 린트 검사
```

---

## 라이선스

MIT 또는 Apache-2.0 (선택)
