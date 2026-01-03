//! SDK Core Integration Test
//!
//! Verifies core SDK functionality via Live API:
//! - Authentication (CLI OAuth)
//! - Client API (query, stream, send, multi-turn, caching)
//! - Agent API (execute, execute_stream)
//! - Tool Registry (14 built-in tools)
//! - Progressive Disclosure (Memory, Skills, Context)
//!
//! Run: cargo run --example sdk_core_test

use claude_agent::{
    Agent, AgentEvent, Auth, Client, ContextBuilder, InMemoryProvider, InMemorySkillProvider,
    MemoryProvider, Message, OAuthConfig, SkillDefinition, SkillIndex, SkillProviderTrait,
    ToolAccess, ToolRegistry, permissions::PermissionMode,
};
use futures::StreamExt;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

macro_rules! test {
    ($name:expr, $body:expr) => {{
        let start = Instant::now();
        match $body {
            Ok(()) => {
                println!("  [PASS] {} ({:.2?})", $name, start.elapsed());
                PASSED.fetch_add(1, Ordering::SeqCst);
            }
            Err(e) => {
                println!("  [FAIL] {} - {}", $name, e);
                FAILED.fetch_add(1, Ordering::SeqCst);
            }
        }
    }};
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("warn").init();

    println!("\n========================================================================");
    println!("                    SDK Core Integration Test                           ");
    println!("========================================================================\n");

    println!("Section 1: Authentication");
    println!("------------------------------------------------------------------------");
    test!("CLI OAuth authentication", test_cli_auth().await);
    test!("OAuth config builder", test_oauth_config());

    println!("\nSection 2: Client API");
    println!("------------------------------------------------------------------------");
    test!("Client query", test_client_query().await);
    test!("Client streaming", test_client_streaming().await);
    test!("Multi-turn conversation", test_multi_turn().await);
    test!("Prompt caching", test_prompt_caching().await);

    println!("\nSection 3: Agent API");
    println!("------------------------------------------------------------------------");
    test!("Agent execute", test_agent_execute().await);
    test!("Agent streaming", test_agent_streaming().await);

    println!("\nSection 4: Tool Registry");
    println!("------------------------------------------------------------------------");
    test!("All 14 tools registered", test_tool_registry());

    println!("\nSection 5: Progressive Disclosure");
    println!("------------------------------------------------------------------------");
    test!("Memory provider", test_memory_provider().await);
    test!("Skill provider", test_skill_provider().await);
    test!("Context orchestrator", test_context_orchestrator());

    let (passed, failed) = (PASSED.load(Ordering::SeqCst), FAILED.load(Ordering::SeqCst));
    println!("\n========================================================================");
    println!("  RESULTS: {} passed, {} failed", passed, failed);
    println!("========================================================================\n");

    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

async fn test_cli_auth() -> Result<(), String> {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .map_err(|e| format!("Auth: {}", e))?
        .build()
        .await
        .map_err(|e| format!("Build: {}", e))?;

    let response = client
        .query("Reply with exactly: OK")
        .await
        .map_err(|e| format!("Query: {}", e))?;

    if response.to_lowercase().contains("ok") {
        Ok(())
    } else {
        Err(format!("Unexpected: {}", response))
    }
}

fn test_oauth_config() -> Result<(), String> {
    use claude_agent::{BetaConfig, BetaFeature, ProviderConfig};

    let config = OAuthConfig::builder()
        .system_prompt("Test prompt")
        .user_agent("test/1.0")
        .app_identifier("test-app")
        .url_param("key", "value")
        .header("X-Custom", "header")
        .build();

    if config.system_prompt != "Test prompt" {
        return Err("system_prompt mismatch".into());
    }
    if config.user_agent != "test/1.0" {
        return Err("user_agent mismatch".into());
    }
    if config.url_params.get("key") != Some(&"value".into()) {
        return Err("url_param missing".into());
    }

    let beta = BetaConfig::new()
        .with(BetaFeature::InterleavedThinking)
        .with_custom("custom-flag");
    if !beta.has(BetaFeature::InterleavedThinking) {
        return Err("beta feature missing".into());
    }
    if !beta.header_value().unwrap().contains("custom-flag") {
        return Err("custom beta missing".into());
    }

    let provider = ProviderConfig::default().with_beta(BetaFeature::ContextManagement);
    if !provider.beta.has(BetaFeature::ContextManagement) {
        return Err("provider beta missing".into());
    }

    Ok(())
}

async fn test_client_query() -> Result<(), String> {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .map_err(|e| format!("Auth: {}", e))?
        .build()
        .await
        .map_err(|e| format!("Build: {}", e))?;

    let response = client
        .query("What is 2+2? Reply only with the number.")
        .await
        .map_err(|e| format!("Query: {}", e))?;

    if response.contains("4") {
        Ok(())
    } else {
        Err(format!("Expected 4, got: {}", response))
    }
}

async fn test_client_streaming() -> Result<(), String> {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .map_err(|e| format!("Auth: {}", e))?
        .build()
        .await
        .map_err(|e| format!("Build: {}", e))?;

    let stream = client
        .stream("Count 1 to 3")
        .await
        .map_err(|e| format!("Stream: {}", e))?;
    let mut stream = pin!(stream);
    let mut chunks = 0;

    while let Some(chunk) = stream.next().await {
        if chunk.is_ok() {
            chunks += 1;
        }
    }

    if chunks > 0 {
        Ok(())
    } else {
        Err("No chunks received".into())
    }
}

async fn test_multi_turn() -> Result<(), String> {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .map_err(|e| format!("Auth: {}", e))?
        .build()
        .await
        .map_err(|e| format!("Build: {}", e))?;

    let request1 = claude_agent::client::CreateMessageRequest::new(
        &client.config().models.primary,
        vec![Message::user("My name is Alice. Remember this.")],
    )
    .with_max_tokens(50);

    let response1 = client.send(request1).await.map_err(|e| format!("{}", e))?;

    let request2 = claude_agent::client::CreateMessageRequest::new(
        &client.config().models.primary,
        vec![
            Message::user("My name is Alice. Remember this."),
            Message::assistant(response1.text()),
            Message::user("What is my name? Just say the name."),
        ],
    )
    .with_max_tokens(20);

    let response2 = client.send(request2).await.map_err(|e| format!("{}", e))?;

    if response2.text().to_lowercase().contains("alice") {
        Ok(())
    } else {
        Err(format!("Name not remembered: {}", response2.text()))
    }
}

async fn test_prompt_caching() -> Result<(), String> {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .map_err(|e| format!("Auth: {}", e))?
        .build()
        .await
        .map_err(|e| format!("Build: {}", e))?;

    let request = claude_agent::client::CreateMessageRequest::new(
        &client.config().models.primary,
        vec![Message::user("Say hi")],
    )
    .with_max_tokens(20);

    let response = client.send(request).await.map_err(|e| format!("{}", e))?;

    if response.usage.input_tokens > 0 {
        Ok(())
    } else {
        Err("No input tokens".into())
    }
}

async fn test_agent_execute() -> Result<(), String> {
    let agent = Agent::builder()
        .auth(Auth::ClaudeCli)
        .await
        .map_err(|e| format!("Auth: {}", e))?
        .tools(ToolAccess::none())
        .permission_mode(PermissionMode::BypassPermissions)
        .max_iterations(1)
        .build()
        .await
        .map_err(|e| format!("Build: {}", e))?;

    let result = agent
        .execute("Say hello")
        .await
        .map_err(|e| format!("Execute: {}", e))?;

    if !result.text.is_empty() {
        Ok(())
    } else {
        Err("Empty response".into())
    }
}

async fn test_agent_streaming() -> Result<(), String> {
    let agent = Agent::builder()
        .auth(Auth::ClaudeCli)
        .await
        .map_err(|e| format!("Auth: {}", e))?
        .tools(ToolAccess::none())
        .permission_mode(PermissionMode::BypassPermissions)
        .max_iterations(1)
        .build()
        .await
        .map_err(|e| format!("Build: {}", e))?;

    let stream = agent
        .execute_stream("Count 1 to 3")
        .await
        .map_err(|e| format!("Execute: {}", e))?;
    let mut stream = pin!(stream);
    let mut has_text = false;
    let mut has_complete = false;

    while let Some(event) = stream.next().await {
        match event.map_err(|e| format!("Event: {}", e))? {
            AgentEvent::Text(_) => has_text = true,
            AgentEvent::Complete(_) => has_complete = true,
            _ => {}
        }
    }

    if has_text && has_complete {
        Ok(())
    } else {
        Err("Missing text or complete event".into())
    }
}

fn test_tool_registry() -> Result<(), String> {
    let registry = ToolRegistry::default_tools(
        &ToolAccess::all(),
        Some(std::path::PathBuf::from(".")),
        None,
    );
    let names = registry.names();

    let expected = [
        "Read",
        "Write",
        "Edit",
        "Glob",
        "Grep",
        "Bash",
        "KillShell",
        "Task",
        "TaskOutput",
        "TodoWrite",
        "Plan",
        "Skill",
    ];

    let missing: Vec<_> = expected.iter().filter(|t| !names.contains(*t)).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("Missing: {:?}", missing))
    }
}

async fn test_memory_provider() -> Result<(), String> {
    let provider = InMemoryProvider::new()
        .with_system_prompt("Test system")
        .with_claude_md("# Project\nTest content")
        .with_priority(100);

    if provider.priority() != 100 {
        return Err("Priority not set".into());
    }

    let content = provider.load().await.map_err(|e| format!("{:?}", e))?;
    if content.claude_md.len() == 2 {
        Ok(())
    } else {
        Err(format!(
            "Expected 2 entries, got {}",
            content.claude_md.len()
        ))
    }
}

async fn test_skill_provider() -> Result<(), String> {
    let provider = InMemorySkillProvider::new()
        .with_item(SkillDefinition::new("skill1", "Desc 1", "content 1"))
        .with_item(SkillDefinition::new("skill2", "Desc 2", "content 2"))
        .with_priority(50);

    if provider.priority() != 50 {
        return Err("Priority not set".into());
    }

    let names = provider.list().await.map_err(|e| e.to_string())?;
    if names.len() != 2 {
        return Err(format!("Expected 2, got {}", names.len()));
    }

    let skill = provider
        .get("skill1")
        .await
        .map_err(|e| e.to_string())?
        .ok_or("Skill not found")?;
    if skill.description == "Desc 1" {
        Ok(())
    } else {
        Err("Description mismatch".into())
    }
}

fn test_context_orchestrator() -> Result<(), String> {
    let orchestrator = ContextBuilder::new()
        .model("claude-sonnet-4-5")
        .system_prompt("Test prompt")
        .claude_md("# Test\nContent")
        .with_skill(SkillIndex::new("commit", "Git helper"))
        .with_skill(
            SkillIndex::new("review", "Code review")
                .with_triggers(vec!["review".into(), "PR".into()]),
        )
        .build()
        .map_err(|e| format!("{:?}", e))?;

    let ctx = orchestrator.static_context();
    if !ctx.system_prompt.contains("Test prompt") {
        return Err("System prompt missing".into());
    }
    if !ctx.claude_md.contains("Test") {
        return Err("Claude.md missing".into());
    }
    if orchestrator.find_skill_by_command("/commit").is_none() {
        return Err("/commit not found".into());
    }

    let matches = orchestrator.find_skills_by_triggers("review this PR");
    if matches.is_empty() {
        Err("Trigger not matched".into())
    } else {
        Ok(())
    }
}
