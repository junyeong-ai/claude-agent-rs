# claude-agent-rs

**Claude Code CLI와 완벽 호환되는 프로덕션 레디 Rust SDK**

[![Crates.io](https://img.shields.io/crates/v/claude-agent.svg)](https://crates.io/crates/claude-agent)
[![Docs.rs](https://docs.rs/claude-agent/badge.svg)](https://docs.rs/claude-agent)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-516%20passing-brightgreen.svg)]()

[English](README.en.md) | 한국어

Claude Code CLI의 OAuth 토큰을 그대로 사용할 수 있는 순수 Rust SDK입니다. Node.js 서브프로세스 없이 Claude API를 직접 호출하며, Prompt Caching과 Progressive Disclosure를 완벽 지원합니다.

---

## 왜 claude-agent-rs인가?

| 특징 | 설명 |
|------|------|
| **Claude Code CLI 호환** | `from_claude_cli()` 한 줄로 기존 인증 사용 |
| **Prompt Caching** | 시스템 프롬프트 자동 캐싱으로 토큰 비용 90% 절감 |
| **순수 Rust** | Node.js 의존성 없음, ~50MB 메모리 |
| **13개 내장 도구** | Read, Write, Edit, Bash, Glob, Grep, Task 등 |
| **Progressive Disclosure** | 필요할 때만 스킬/규칙 로딩으로 컨텍스트 최적화 |
| **멀티 클라우드** | Anthropic, AWS Bedrock, Google Vertex AI, Azure Foundry |

---

## 설치

```toml
[dependencies]
claude-agent = "0.1"
tokio = { version = "1", features = ["full"] }
```

---

## 빠른 시작

### 1. 간단한 쿼리

```rust
use claude_agent::query;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let response = query("Rust의 장점은?").await?;
    println!("{response}");
    Ok(())
}
```

### 2. 스트리밍 응답

```rust
use claude_agent::stream;
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let stream = stream("양자 컴퓨팅을 설명해줘").await?;
    let mut stream = pin!(stream);

    while let Some(chunk) = stream.next().await {
        print!("{}", chunk?);
    }
    Ok(())
}
```

### 3. Agent + Tools (핵심 사용법)

```rust
use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> claude_agent::Result<()> {
    let agent = Agent::builder()
        .from_claude_cli()              // Claude Code CLI 인증 사용
        .tools(ToolAccess::all())       // 13개 도구 모두 활성화
        .working_dir("./my-project")
        .max_iterations(10)
        .build()
        .await?;

    let stream = agent.execute_stream("main.rs의 버그를 수정해줘").await?;
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{text}"),
            AgentEvent::ToolStart { name, .. } => eprintln!("\n🔧 [{name}]"),
            AgentEvent::ToolEnd { .. } => eprintln!(" ✓"),
            AgentEvent::Complete(result) => {
                eprintln!("\n✅ 완료: {} tokens, {} tool calls",
                    result.total_tokens(), result.tool_calls);
            }
            _ => {}
        }
    }
    Ok(())
}
```

---

## 인증 방법

### Claude Code CLI (권장)

기존 Claude Code CLI 인증을 그대로 사용합니다:

```rust
let client = Client::builder()
    .from_claude_cli()
    .build()?;

// Agent도 동일하게
let agent = Agent::builder()
    .from_claude_cli()
    .build()
    .await?;
```

**필요 조건**: `claude --version`으로 CLI가 인증되어 있어야 합니다.

**자동 포함 기능**:
- OAuth Bearer 토큰
- Prompt Caching (`cache_control: ephemeral`)
- 필수 베타 플래그 (`claude-code-20250219`, `oauth-2025-04-20`)

### API Key

```rust
let client = Client::builder()
    .api_key("sk-ant-...")
    .build()?;
```

### 환경 변수

```rust
// ANTHROPIC_API_KEY 자동 사용
let client = Client::from_env()?;
```

### 클라우드 공급자

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

## 13개 내장 도구

### 파일 도구
| 도구 | 설명 |
|------|------|
| `Read` | 파일 읽기 (이미지, PDF, Jupyter 노트북 지원) |
| `Write` | 파일 생성/덮어쓰기 |
| `Edit` | 문자열 치환 기반 정밀 편집 |
| `Glob` | 패턴 기반 파일 검색 (`**/*.rs`) |
| `Grep` | 정규식 내용 검색 (ripgrep 기반) |
| `NotebookEdit` | Jupyter 노트북 셀 편집 |

### 실행 도구
| 도구 | 설명 |
|------|------|
| `Bash` | 셸 명령 실행 (타임아웃, 백그라운드 지원) |
| `KillShell` | 백그라운드 프로세스 종료 |

### 에이전트 도구
| 도구 | 설명 |
|------|------|
| `Task` | 서브에이전트 생성 및 실행 |
| `TaskOutput` | 백그라운드 작업 결과 조회 |
| `TodoWrite` | 작업 목록 관리 |
| `Skill` | 등록된 스킬 실행 |
| `WebFetch` | URL 콘텐츠 가져오기 |

### 도구 접근 제어

```rust
// 모든 도구 활성화
Agent::builder().tools(ToolAccess::all())

// 특정 도구만 허용
Agent::builder().tools(ToolAccess::only(["Read", "Grep", "Glob"]))

// 특정 도구 제외 (보안)
Agent::builder().tools(ToolAccess::except(["Bash", "Write"]))
```

---

## Progressive Disclosure

필요한 시점에 스킬과 규칙을 동적으로 로딩하여 컨텍스트 윈도우를 효율적으로 사용합니다.

### 스킬 시스템

```rust
use claude_agent::{Agent, SkillDefinition, ToolAccess};

let agent = Agent::builder()
    .from_claude_cli()
    .skill(SkillDefinition::new(
        "deploy",
        "프로덕션 배포",
        "배포 프로세스: $ARGUMENTS\n1. 테스트 실행\n2. 빌드\n3. 배포",
    ).with_trigger("deploy").with_trigger("배포"))
    .tools(ToolAccess::only(["Skill", "Bash"]))
    .build()
    .await?;
```

### 트리거 기반 활성화

스킬은 명시적 호출 또는 트리거 키워드로 활성화됩니다:

```rust
// 명시적: /deploy production
// 트리거: "프로덕션에 배포해줘" → "배포" 키워드 감지 → deploy 스킬 활성화
```

### 슬래시 명령

`.claude/commands/deploy.md`:

```yaml
---
description: 프로덕션 배포
allowed-tools:
  - Bash
  - Read
---

$ARGUMENTS 환경에 배포합니다.
```

---

## Prompt Caching

Claude Code CLI 인증 사용 시 자동으로 Prompt Caching이 활성화됩니다.

### 동작 방식

```
첫 번째 요청: cache_creation_input_tokens (캐시 생성)
두 번째 요청: cache_read_input_tokens (캐시 히트, 90% 비용 절감)
```

### 캐시 통계 확인

```rust
use claude_agent::session::CacheStats;

let stats = CacheStats::default();
stats.update(1000, 0);  // cache_read: 1000, cache_creation: 0

println!("Cache hit rate: {:.1}%", stats.hit_rate() * 100.0);
println!("Tokens saved: {}", stats.tokens_saved());
```

---

## 메모리 시스템

`CLAUDE.md` 파일을 자동으로 로드하여 프로젝트 컨텍스트를 제공합니다.

```markdown
# 프로젝트 가이드

@import ./docs/architecture.md
@import ~/global-rules.md

## 코딩 규칙
- Rust 2021 Edition 사용
- 모든 pub 함수에 문서화 필수
```

**로딩 우선순위**: `~/.claude/CLAUDE.md` → 프로젝트 루트 → 현재 디렉토리

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
```

---

## 환경 변수

| 변수 | 설명 |
|------|------|
| `ANTHROPIC_API_KEY` | Anthropic API 키 |
| `ANTHROPIC_MODEL` | 기본 모델 (default: `claude-sonnet-4-5`) |
| `CLAUDE_CODE_USE_BEDROCK` | AWS Bedrock 사용 |
| `CLAUDE_CODE_USE_VERTEX` | Google Vertex AI 사용 |
| `CLAUDE_CODE_USE_FOUNDRY` | Azure Foundry 사용 |

---

## 예제

```bash
# 기본 쿼리
cargo run --example simple_query

# 스트리밍 응답
cargo run --example streaming

# Agent 실행 루프
cargo run --example agent_loop

# 전체 도구 테스트
cargo run --example comprehensive_test
```

---

## 테스트

```bash
# 단위 테스트 (인증 불필요)
cargo test

# CLI 인증 테스트 포함
cargo test -- --ignored

# 전체 검증
cargo run --example comprehensive_test
```

**테스트 현황**: 516개 테스트 통과

---

## License

MIT 또는 Apache-2.0 (선택)
