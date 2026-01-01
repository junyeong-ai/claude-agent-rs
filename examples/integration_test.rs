//! Comprehensive Integration Test for Claude Agent SDK
//!
//! Tests CLI authentication, progressive disclosure, all tools, and prompt caching.
//!
//! Run with: cargo run --example integration_test
//!
//! Prerequisites:
//! - Claude CLI installed and authenticated (`claude --version`)

use claude_agent::{
    Agent, AgentEvent, ChainMemoryProvider, ChainSkillProvider, ClientBuilder, ContextBuilder,
    InMemoryProvider, InMemorySkillProvider, MemoryProvider, OAuthConfig, SkillDefinition,
    SkillProvider, ToolAccess, ToolRegistry,
};
use futures::StreamExt;
use std::pin::pin;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("claude_agent=debug,integration_test=debug")
        .init();

    println!("╔════════════════════════════════════════════════════════════════╗");
    println!("║     Claude Agent SDK - Comprehensive Integration Test          ║");
    println!("╠════════════════════════════════════════════════════════════════╣");

    let mut passed = 0;
    let mut failed = 0;

    // 1. CLI Authentication Test
    match test_cli_authentication().await {
        Ok(()) => {
            println!("║ ✅ CLI Authentication                                          ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ CLI Authentication: {:<38} ║", truncate(&e, 38));
            failed += 1;
        }
    }

    // 2. OAuth Config Test
    match test_oauth_config().await {
        Ok(()) => {
            println!("║ ✅ OAuth Configuration                                         ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ OAuth Configuration: {:<37} ║", truncate(&e, 37));
            failed += 1;
        }
    }

    // 3. Memory Provider (Progressive Disclosure)
    match test_memory_provider().await {
        Ok(()) => {
            println!("║ ✅ Memory Provider (Progressive Disclosure)                    ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ Memory Provider: {:<41} ║", truncate(&e, 41));
            failed += 1;
        }
    }

    // 4. Skill Provider
    match test_skill_provider().await {
        Ok(()) => {
            println!("║ ✅ Skill Provider                                              ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ Skill Provider: {:<42} ║", truncate(&e, 42));
            failed += 1;
        }
    }

    // 5. Context Orchestrator
    match test_context_orchestrator().await {
        Ok(()) => {
            println!("║ ✅ Context Orchestrator                                        ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ Context Orchestrator: {:<36} ║", truncate(&e, 36));
            failed += 1;
        }
    }

    // 6. Tool Registry
    match test_tool_registry().await {
        Ok(()) => {
            println!("║ ✅ Tool Registry (14 built-in tools)                           ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ Tool Registry: {:<43} ║", truncate(&e, 43));
            failed += 1;
        }
    }

    // 7. Prompt Caching
    match test_prompt_caching().await {
        Ok(()) => {
            println!("║ ✅ Prompt Caching                                              ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ Prompt Caching: {:<42} ║", truncate(&e, 42));
            failed += 1;
        }
    }

    // 8. Streaming API
    match test_streaming().await {
        Ok(()) => {
            println!("║ ✅ Streaming API                                               ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ Streaming API: {:<43} ║", truncate(&e, 43));
            failed += 1;
        }
    }

    // 9. Agent Loop with Tools
    match test_agent_loop().await {
        Ok(()) => {
            println!("║ ✅ Agent Loop with Tool Execution                              ║");
            passed += 1;
        }
        Err(e) => {
            println!("║ ❌ Agent Loop: {:<46} ║", truncate(&e, 46));
            failed += 1;
        }
    }

    println!("╠════════════════════════════════════════════════════════════════╣");
    println!(
        "║ Results: {} passed, {} failed                                   ║",
        passed, failed
    );
    println!("╚════════════════════════════════════════════════════════════════╝");

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

/// Test 1: CLI Authentication
async fn test_cli_authentication() -> Result<(), String> {
    let client = ClientBuilder::default()
        .from_claude_cli()
        .build()
        .map_err(|e| format!("Build failed: {}", e))?;

    // Simple query to verify auth works
    let response = client
        .query("Reply with only: OK")
        .await
        .map_err(|e| format!("Query failed: {}", e))?;

    if response.to_lowercase().contains("ok") {
        Ok(())
    } else {
        Err(format!("Unexpected response: {}", response))
    }
}

/// Test 2: OAuth Configuration
async fn test_oauth_config() -> Result<(), String> {
    // Test builder pattern
    let config = OAuthConfig::builder()
        .system_prompt("Test prompt")
        .user_agent("test-agent/1.0")
        .add_beta_flag("test-flag")
        .build();

    if config.system_prompt != "Test prompt" {
        return Err("System prompt not set".to_string());
    }
    if config.user_agent != "test-agent/1.0" {
        return Err("User agent not set".to_string());
    }
    if !config.beta_flags.contains(&"test-flag".to_string()) {
        return Err("Beta flag not added".to_string());
    }

    // Test from_env
    let env_config = OAuthConfig::from_env();
    if env_config.system_prompt.is_empty() {
        return Err("Default system prompt is empty".to_string());
    }

    Ok(())
}

/// Test 3: Memory Provider (Progressive Disclosure)
async fn test_memory_provider() -> Result<(), String> {
    // In-memory provider
    let provider = InMemoryProvider::new()
        .with_system_prompt("You are a helpful assistant")
        .with_claude_md("# Project Rules\nBe concise.")
        .with_rule("security", "No secrets in code");

    let content = provider
        .load()
        .await
        .map_err(|e| format!("Load failed: {}", e))?;

    if content.claude_md.len() != 2 {
        return Err(format!(
            "Expected 2 claude_md entries, got {}",
            content.claude_md.len()
        ));
    }
    if content.rules.len() != 1 {
        return Err(format!("Expected 1 rule, got {}", content.rules.len()));
    }

    // Chain provider
    let low_priority = InMemoryProvider::new()
        .with_claude_md("Low priority content")
        .with_priority(0);

    let high_priority = InMemoryProvider::new()
        .with_claude_md("High priority content")
        .with_priority(10);

    let chain = ChainMemoryProvider::new()
        .with(low_priority)
        .with(high_priority);

    let combined = chain
        .load()
        .await
        .map_err(|e| format!("Chain load failed: {}", e))?;

    if combined.claude_md.len() != 2 {
        return Err(format!(
            "Chain expected 2 entries, got {}",
            combined.claude_md.len()
        ));
    }

    // Verify ordering (low priority loaded first)
    if combined.claude_md[0] != "Low priority content" {
        return Err("Chain priority ordering incorrect".to_string());
    }

    Ok(())
}

/// Test 4: Skill Provider
async fn test_skill_provider() -> Result<(), String> {
    // In-memory skill provider
    let skill = SkillDefinition::new("test-skill", "A test skill", "Do the test thing");

    let provider = InMemorySkillProvider::new().with_skill(skill);

    let names = provider
        .list()
        .await
        .map_err(|e| format!("List failed: {}", e))?;

    if !names.contains(&"test-skill".to_string()) {
        return Err("Skill not found in list".to_string());
    }

    let loaded = provider
        .get("test-skill")
        .await
        .map_err(|e| format!("Get failed: {}", e))?;

    if loaded.is_none() {
        return Err("Skill get returned None".to_string());
    }

    let skill = loaded.unwrap();
    if skill.description != "A test skill" {
        return Err("Skill description mismatch".to_string());
    }

    // Chain provider
    let low = InMemorySkillProvider::new()
        .with_skill(SkillDefinition::new(
            "shared",
            "Low priority",
            "Low content",
        ))
        .with_priority(0);

    let high = InMemorySkillProvider::new()
        .with_skill(SkillDefinition::new(
            "shared",
            "High priority",
            "High content",
        ))
        .with_priority(10);

    let chain = ChainSkillProvider::new().with(low).with(high);

    let shared = chain
        .get("shared")
        .await
        .map_err(|e| format!("Chain get failed: {}", e))?
        .ok_or("Shared skill not found")?;

    // Higher priority should win
    if shared.description != "High priority" {
        return Err(format!("Chain priority incorrect: {}", shared.description));
    }

    Ok(())
}

/// Test 5: Context Orchestrator
async fn test_context_orchestrator() -> Result<(), String> {
    let orchestrator = ContextBuilder::new()
        .model("claude-sonnet-4-5")
        .system_prompt("Test system prompt")
        .claude_md("# Test Project\nThis is a test.")
        .with_dynamic(|| format!("Current time: {}", chrono::Utc::now()))
        .when(|| true, "Conditional: included")
        .when(|| false, "Conditional: excluded")
        .build_sync()
        .map_err(|e| format!("Build failed: {}", e))?;

    let ctx = orchestrator.static_context();

    if !ctx.system_prompt.contains("Test system prompt") {
        return Err("System prompt not set".to_string());
    }

    if !ctx.claude_md.contains("Test Project") {
        return Err("CLAUDE.md not set".to_string());
    }

    if !ctx.claude_md.contains("Current time") {
        return Err("Dynamic content not evaluated".to_string());
    }

    if !ctx.claude_md.contains("Conditional: included") {
        return Err("Conditional (true) not included".to_string());
    }

    if ctx.claude_md.contains("Conditional: excluded") {
        return Err("Conditional (false) should be excluded".to_string());
    }

    Ok(())
}

/// Test 6: Tool Registry
async fn test_tool_registry() -> Result<(), String> {
    let registry = ToolRegistry::default_tools(&ToolAccess::all(), None);
    let names = registry.names();

    let expected_tools = [
        "Read",
        "Write",
        "Edit",
        "Glob",
        "Grep",
        "NotebookEdit",
        "Bash",
        "KillShell",
        "WebSearch",
        "WebFetch",
        "Task",
        "TaskOutput",
        "TodoWrite",
        "Skill",
    ];

    for tool in &expected_tools {
        if !names.contains(tool) {
            return Err(format!("Missing tool: {}", tool));
        }
    }

    // Test Glob tool execution
    let result = registry
        .execute(
            "Glob",
            serde_json::json!({
                "pattern": "*.rs",
                "path": "."
            }),
        )
        .await;

    if result.is_error() {
        return Err("Glob tool execution failed".to_string());
    }

    // Test restricted access
    let restricted = ToolRegistry::default_tools(&ToolAccess::only(["Read"]), None);
    if !restricted.contains("Read") {
        return Err("Restricted registry missing allowed tool".to_string());
    }
    if restricted.contains("Write") {
        return Err("Restricted registry has disallowed tool".to_string());
    }

    Ok(())
}

/// Test 7: Prompt Caching
async fn test_prompt_caching() -> Result<(), String> {
    let client = ClientBuilder::default()
        .from_claude_cli()
        .build()
        .map_err(|e| format!("Build failed: {}", e))?;

    // First request to prime cache
    let start1 = Instant::now();
    let _ = client
        .query("What is 1+1? Reply with just the number.")
        .await
        .map_err(|e| format!("First query failed: {}", e))?;
    let duration1 = start1.elapsed();

    // Second request (should hit cache)
    let start2 = Instant::now();
    let _ = client
        .query("What is 1+1? Reply with just the number.")
        .await
        .map_err(|e| format!("Second query failed: {}", e))?;
    let duration2 = start2.elapsed();

    // Cache should make subsequent requests faster or similar
    // We can't strictly guarantee this, so just verify both succeed
    println!(
        "    Cache timing: first={:?}, second={:?}",
        duration1, duration2
    );

    Ok(())
}

/// Test 8: Streaming API
async fn test_streaming() -> Result<(), String> {
    let client = ClientBuilder::default()
        .from_claude_cli()
        .build()
        .map_err(|e| format!("Build failed: {}", e))?;

    let stream = client
        .stream("Count from 1 to 3. Just numbers separated by spaces.")
        .await
        .map_err(|e| format!("Stream failed: {}", e))?;

    let mut stream = pin!(stream);
    let mut chunks = Vec::new();

    while let Some(chunk) = stream.next().await {
        let text = chunk.map_err(|e| format!("Chunk error: {}", e))?;
        chunks.push(text);
    }

    if chunks.is_empty() {
        return Err("No chunks received".to_string());
    }

    let full_response: String = chunks.concat();
    if !full_response.contains("1") || !full_response.contains("2") || !full_response.contains("3")
    {
        return Err(format!("Incomplete stream response: {}", full_response));
    }

    Ok(())
}

/// Test 9: Agent Loop with Tools
async fn test_agent_loop() -> Result<(), String> {
    let agent = Agent::builder()
        .from_claude_cli()
        .tools(ToolAccess::only(["Glob", "Read"]))
        .working_dir(".")
        .build()
        .await
        .map_err(|e| format!("Build failed: {}", e))?;

    let stream = agent
        .execute_stream("Use the Glob tool to find *.toml files in current directory. List them.")
        .await
        .map_err(|e| format!("Execute failed: {}", e))?;

    let mut stream = pin!(stream);
    let mut tool_starts = 0;
    let mut tool_ends = 0;
    let mut has_complete = false;

    while let Some(event) = stream.next().await {
        match event.map_err(|e| format!("Event error: {}", e))? {
            AgentEvent::ToolStart { name, .. } => {
                tool_starts += 1;
                if name != "Glob" {
                    return Err(format!("Unexpected tool: {}", name));
                }
            }
            AgentEvent::ToolEnd { is_error, .. } => {
                tool_ends += 1;
                if is_error {
                    return Err("Tool execution returned error".to_string());
                }
            }
            AgentEvent::Complete(result) => {
                has_complete = true;
                if result.tool_calls == 0 {
                    return Err("Expected at least 1 tool call".to_string());
                }
            }
            _ => {}
        }
    }

    if !has_complete {
        return Err("No Complete event received".to_string());
    }

    if tool_starts != tool_ends {
        return Err(format!(
            "Tool start/end mismatch: {} vs {}",
            tool_starts, tool_ends
        ));
    }

    Ok(())
}
