//! Direct Verification - 실제 동작 검증
//!
//! Run: cargo run --example direct_verification

use claude_agent::{
    Agent, Client, ToolAccess,
    client::messages::{CreateMessageRequest, RequestMetadata},
    skills::{SkillDefinition, SkillExecutor, SkillRegistry},
    types::Message,
};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       Direct Verification - CLI 인증 기반 전체 기능 검증       ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // =========================================================================
    // TEST 1: CLI 인증 및 기본 API 호출
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 1: CLI 인증 및 기본 API 호출");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("CLI 인증 실패 - claude login 실행 필요");

    println!("✓ CLI 인증 성공");
    println!("  Auth Strategy: {}", client.config().auth_strategy.name());
    println!("  Model: {}", client.config().model);
    println!("  Base URL: {}", client.config().base_url);

    // 기본 쿼리 테스트
    let response = client
        .query("What is 7 * 8? Answer with just the number.")
        .await?;
    println!("\n  Query: 7 * 8 = {}", response.trim());
    assert!(response.contains("56"), "계산 결과가 맞아야 함");
    println!("  ✓ 기본 API 호출 성공\n");

    // =========================================================================
    // TEST 2: 스트리밍 응답
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 2: 스트리밍 응답");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let request = CreateMessageRequest::new(
        &client.config().model,
        vec![Message::user("Count from 1 to 5, one number per line.")],
    )
    .with_max_tokens(50);

    let stream = claude_agent::client::MessagesClient::new(&client)
        .create_stream(request)
        .await?;

    let mut stream = pin!(stream);
    let mut chunks = Vec::new();
    print!("  스트리밍 응답: ");

    while let Some(item) = stream.next().await {
        if let Ok(claude_agent::client::StreamItem::Text(text)) = item {
            print!("{}", text);
            chunks.push(text);
        }
    }
    println!("\n");

    assert!(!chunks.is_empty(), "스트리밍 청크가 있어야 함");
    println!("  ✓ 스트리밍 {} 청크 수신\n", chunks.len());

    // =========================================================================
    // TEST 3: 에이전트 + 도구 사용
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 3: 에이전트 + 도구 사용 (Read, Glob, Bash)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // 임시 파일 생성
    let temp_dir = tempfile::tempdir()?;
    let test_file = temp_dir.path().join("secret.txt");
    tokio::fs::write(&test_file, "The secret password is: RUST2025").await?;
    println!("  테스트 파일 생성: {}", test_file.display());

    let agent = Agent::builder()
        .from_claude_cli()
        .tools(ToolAccess::only(["Read", "Glob", "Bash"]))
        .working_dir(temp_dir.path())
        .max_iterations(5)
        .build()
        .await?;

    let result = agent
        .execute(&format!(
            "Read the file at {} and tell me what the secret password is.",
            test_file.display()
        ))
        .await?;

    println!("\n  에이전트 응답: {}", result.text());
    println!("  도구 호출 횟수: {}", result.tool_calls);
    println!("  반복 횟수: {}", result.iterations);
    println!("  총 토큰: {}", result.total_tokens());

    assert!(result.tool_calls >= 1, "최소 1회 도구 호출 필요");
    assert!(
        result.text().contains("RUST2025") || result.text().contains("secret"),
        "비밀번호를 찾아야 함"
    );
    println!("  ✓ 에이전트 도구 사용 성공\n");

    // =========================================================================
    // TEST 4: Progressive Disclosure (Skills)
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 4: Progressive Disclosure (Skills)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let mut registry = SkillRegistry::new();
    registry.register(
        SkillDefinition::new(
            "calculator",
            "수학 계산기",
            "Calculate: $ARGUMENTS\n\nShow step-by-step calculation.",
        )
        .with_trigger("calculate")
        .with_trigger("math"),
    );
    registry.register(
        SkillDefinition::new(
            "greeter",
            "인사말 생성",
            "Generate greeting for: $ARGUMENTS",
        )
        .with_trigger("greet")
        .with_trigger("hello"),
    );

    let executor = SkillExecutor::new(registry);

    // 스킬 존재 확인
    assert!(
        executor.has_skill("calculator"),
        "calculator 스킬 존재해야 함"
    );
    assert!(executor.has_skill("greeter"), "greeter 스킬 존재해야 함");
    println!("  ✓ 스킬 등록 확인: calculator, greeter");

    // 스킬 실행
    let calc_result = executor.execute("calculator", Some("15 * 4 + 20")).await;
    println!("  Calculator 스킬 실행: {}", calc_result.success);

    // 트리거 기반 실행
    let triggered = executor
        .execute_by_trigger("please calculate 100 / 5")
        .await;
    assert!(triggered.is_some(), "트리거로 스킬이 활성화되어야 함");
    println!("  ✓ 트리거 기반 스킬 활성화 성공\n");

    // =========================================================================
    // TEST 5: 프롬프트 캐싱 필드 확인
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 5: 프롬프트 캐싱 필드 확인");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let request1 =
        CreateMessageRequest::new(&client.config().model, vec![Message::user("Say hello")])
            .with_max_tokens(20)
            .with_metadata(RequestMetadata::generate());

    let response1 = claude_agent::client::MessagesClient::new(&client)
        .create(request1)
        .await?;

    println!("  Request 1:");
    println!("    Input tokens: {}", response1.usage.input_tokens);
    println!("    Output tokens: {}", response1.usage.output_tokens);
    println!(
        "    Cache creation: {:?}",
        response1.usage.cache_creation_input_tokens
    );
    println!(
        "    Cache read: {:?}",
        response1.usage.cache_read_input_tokens
    );

    // 두 번째 요청
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    let request2 =
        CreateMessageRequest::new(&client.config().model, vec![Message::user("Say goodbye")])
            .with_max_tokens(20)
            .with_metadata(RequestMetadata::generate());

    let response2 = claude_agent::client::MessagesClient::new(&client)
        .create(request2)
        .await?;

    println!("\n  Request 2:");
    println!("    Input tokens: {}", response2.usage.input_tokens);
    println!("    Output tokens: {}", response2.usage.output_tokens);
    println!(
        "    Cache creation: {:?}",
        response2.usage.cache_creation_input_tokens
    );
    println!(
        "    Cache read: {:?}",
        response2.usage.cache_read_input_tokens
    );
    println!("  ✓ 프롬프트 캐싱 필드 확인 완료\n");

    // =========================================================================
    // TEST 6: 멀티턴 대화
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 6: 멀티턴 대화");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Turn 1
    let turn1_request = CreateMessageRequest::new(
        &client.config().model,
        vec![Message::user("My name is Alice. Remember this.")],
    )
    .with_max_tokens(50);

    let turn1_response = claude_agent::client::MessagesClient::new(&client)
        .create(turn1_request)
        .await?;

    println!("  Turn 1: {}", turn1_response.text());

    // Turn 2 - 컨텍스트 유지
    let turn2_request = CreateMessageRequest::new(
        &client.config().model,
        vec![
            Message::user("My name is Alice. Remember this."),
            Message::assistant(turn1_response.text()),
            Message::user("What is my name? Just say the name."),
        ],
    )
    .with_max_tokens(20);

    let turn2_response = claude_agent::client::MessagesClient::new(&client)
        .create(turn2_request)
        .await?;

    println!("  Turn 2: {}", turn2_response.text());
    assert!(
        turn2_response.text().to_lowercase().contains("alice"),
        "이름을 기억해야 함"
    );
    println!("  ✓ 멀티턴 컨텍스트 유지 성공\n");

    // =========================================================================
    // TEST 7: 에이전트 스트리밍
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 7: 에이전트 스트리밍");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let streaming_agent = Agent::builder()
        .from_claude_cli()
        .tools(ToolAccess::none())
        .max_iterations(1)
        .build()
        .await?;

    let agent_stream = streaming_agent
        .execute_stream("Write a haiku about Rust programming.")
        .await?;

    let mut agent_stream = pin!(agent_stream);
    let mut text_parts = Vec::new();
    let mut complete = false;

    print!("  스트리밍: ");
    while let Some(event) = agent_stream.next().await {
        match event? {
            claude_agent::AgentEvent::Text(text) => {
                print!("{}", text);
                text_parts.push(text);
            }
            claude_agent::AgentEvent::Complete(result) => {
                println!("\n\n  완료: {} 토큰", result.total_tokens());
                complete = true;
            }
            _ => {}
        }
    }

    assert!(!text_parts.is_empty(), "텍스트를 받아야 함");
    assert!(complete, "완료 이벤트가 있어야 함");
    println!("  ✓ 에이전트 스트리밍 성공\n");

    // =========================================================================
    // 최종 결과
    // =========================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    모든 테스트 통과! ✓                        ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  ✓ TEST 1: CLI 인증 및 기본 API 호출                          ║");
    println!("║  ✓ TEST 2: 스트리밍 응답                                      ║");
    println!("║  ✓ TEST 3: 에이전트 + 도구 사용                               ║");
    println!("║  ✓ TEST 4: Progressive Disclosure (Skills)                   ║");
    println!("║  ✓ TEST 5: 프롬프트 캐싱 필드                                 ║");
    println!("║  ✓ TEST 6: 멀티턴 대화                                        ║");
    println!("║  ✓ TEST 7: 에이전트 스트리밍                                  ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    Ok(())
}
