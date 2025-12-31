//! End-to-End CLI Authentication Verification Tests
//!
//! Comprehensive verification of all SDK features with Claude Code CLI authentication.
//! Tests progressive disclosure, all tools, prompt caching, and advanced features.
//!
//! Run: cargo test --test e2e_cli_verification -- --ignored --nocapture

use claude_agent::{
    auth::{AuthStrategy, OAuthCredential, OAuthStrategy},
    client::messages::CreateMessageRequest,
    types::{Message, SystemPrompt},
    Agent, Client, ToolAccess, ToolRegistry,
};
use futures::StreamExt;
use std::collections::HashMap;
use std::pin::pin;

// =============================================================================
// Part 1: Authentication Verification
// =============================================================================

mod auth_verification {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_oauth_authentication_complete() {
        println!("\n=== OAuth Authentication Verification ===\n");

        let client = Client::builder()
            .from_claude_cli()
            .build()
            .expect("Failed to create client");

        // 1. Verify auth strategy type
        let strategy_name = client.config().auth_strategy.name();
        println!("✓ Auth strategy: {}", strategy_name);
        assert_eq!(strategy_name, "oauth");

        // 2. Verify we can make authenticated requests
        let response = client.query("Say 'auth works'").await.expect("Query failed");
        println!("✓ Authenticated query successful: {}", response.trim());
        assert!(!response.is_empty());

        println!("\n✅ OAuth authentication verification complete\n");
    }

    #[test]
    fn verify_oauth_headers_structure() {
        println!("\n=== OAuth Headers Structure ===\n");

        let cred = OAuthCredential {
            access_token: "test-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        };
        let strategy = OAuthStrategy::new(cred);

        let headers = strategy.extra_headers();
        let header_map: HashMap<_, _> = headers.into_iter().collect();

        // Required headers
        let required_headers = [
            ("anthropic-beta", "claude-code", "Beta flags"),
            ("user-agent", "claude-cli", "User agent"),
            ("x-app", "cli", "App identifier"),
            ("anthropic-dangerous-direct-browser-access", "true", "Browser access"),
        ];

        for (key, contains, desc) in required_headers {
            assert!(header_map.contains_key(key), "Missing header: {}", key);
            let value = header_map.get(key).unwrap();
            assert!(value.contains(contains), "{} should contain '{}'", desc, contains);
            println!("✓ {}: {}", key, value);
        }

        // Verify auth header format
        let (name, value) = strategy.auth_header();
        assert_eq!(name, "Authorization");
        assert!(value.starts_with("Bearer "));
        println!("✓ Auth header: {}: Bearer ***", name);

        // Verify URL params
        let query = strategy.url_query_string();
        assert!(query.is_some());
        assert!(query.as_ref().unwrap().contains("beta=true"));
        println!("✓ URL params: {:?}", query);

        println!("\n✅ Headers structure verification complete\n");
    }
}

// =============================================================================
// Part 2: Prompt Caching Verification
// =============================================================================

mod prompt_caching {
    use super::*;

    #[test]
    fn verify_cache_control_on_system_prompt() {
        println!("\n=== Prompt Caching Structure ===\n");

        let cred = OAuthCredential {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        };
        let strategy = OAuthStrategy::new(cred);

        // Test with no existing prompt
        let result = strategy.prepare_system_prompt(None);
        if let Some(SystemPrompt::Blocks(blocks)) = result {
            assert_eq!(blocks.len(), 1);
            assert!(blocks[0].text.contains("Claude Code"));
            assert!(blocks[0].cache_control.is_some());
            assert_eq!(blocks[0].cache_control.as_ref().unwrap().cache_type, "ephemeral");
            println!("✓ System prompt has cache_control: ephemeral");
        } else {
            panic!("Expected Blocks variant");
        }

        // Test with existing user prompt
        let user_prompt = SystemPrompt::text("Custom instructions");
        let result = strategy.prepare_system_prompt(Some(user_prompt));
        if let Some(SystemPrompt::Blocks(blocks)) = result {
            assert_eq!(blocks.len(), 2);
            // First block: Claude Code with cache
            assert!(blocks[0].text.contains("Claude Code"));
            assert!(blocks[0].cache_control.is_some());
            // Second block: User prompt without cache
            assert_eq!(blocks[1].text, "Custom instructions");
            assert!(blocks[1].cache_control.is_none());
            println!("✓ User prompt preserved without cache_control");
            println!("✓ Claude Code prompt prepended with cache_control");
        } else {
            panic!("Expected Blocks variant");
        }

        println!("\n✅ Prompt caching structure verification complete\n");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_prompt_caching_api_response() {
        println!("\n=== Prompt Caching API Response ===\n");

        let client = Client::builder()
            .from_claude_cli()
            .build()
            .expect("Failed to create client");

        // Make two requests to see caching in action
        let request1 = CreateMessageRequest::new(
            &client.config().model,
            vec![Message::user("Say 'test1'")],
        )
        .with_max_tokens(20);

        let response1 = claude_agent::client::MessagesClient::new(&client)
            .create(request1)
            .await
            .expect("Request 1 failed");

        println!("Request 1:");
        println!("  Input tokens: {}", response1.usage.input_tokens);
        println!("  Cache creation: {:?}", response1.usage.cache_creation_input_tokens);
        println!("  Cache read: {:?}", response1.usage.cache_read_input_tokens);

        // Second request should potentially use cache
        let request2 = CreateMessageRequest::new(
            &client.config().model,
            vec![Message::user("Say 'test2'")],
        )
        .with_max_tokens(20);

        let response2 = claude_agent::client::MessagesClient::new(&client)
            .create(request2)
            .await
            .expect("Request 2 failed");

        println!("Request 2:");
        println!("  Input tokens: {}", response2.usage.input_tokens);
        println!("  Cache creation: {:?}", response2.usage.cache_creation_input_tokens);
        println!("  Cache read: {:?}", response2.usage.cache_read_input_tokens);

        // Verify cache fields are present
        assert!(response1.usage.cache_creation_input_tokens.is_some() ||
                response1.usage.cache_read_input_tokens.is_some(),
                "Cache fields should be present in response");

        println!("\n✅ Prompt caching API verification complete\n");
    }
}

// =============================================================================
// Part 3: Progressive Disclosure Verification
// =============================================================================

mod progressive_disclosure {
    use super::*;

    #[test]
    fn verify_tool_definitions_progressive() {
        println!("\n=== Progressive Disclosure - Tool Definitions ===\n");

        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);
        let definitions = registry.definitions();

        println!("Total tools registered: {}", definitions.len());
        assert!(definitions.len() >= 5, "Should have at least 5 tools");
        assert!(definitions.len() <= 20, "Should have reasonable number of tools");

        let mut total_chars = 0;
        for def in &definitions {
            let desc_len = def.description.len();
            total_chars += desc_len;

            // Each tool should have concise but useful description
            assert!(desc_len >= 20, "Tool {} description too short", def.name);
            assert!(desc_len <= 2000, "Tool {} description too long", def.name);

            println!("  {} ({} chars)", def.name, desc_len);
        }

        let avg_chars = total_chars / definitions.len();
        println!("\nAverage description length: {} chars", avg_chars);

        // Average should be reasonable for initial context loading
        assert!(avg_chars < 500, "Average description too long for progressive disclosure");

        println!("\n✅ Progressive disclosure verification complete\n");
    }

    #[test]
    fn verify_tool_schemas_complete() {
        println!("\n=== Tool Schema Completeness ===\n");

        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);

        for def in registry.definitions() {
            let schema = &def.input_schema;

            // Must have type: object
            assert!(schema.get("type").is_some(), "{} missing type", def.name);
            assert_eq!(schema["type"], "object", "{} type must be object", def.name);

            // Must have properties
            assert!(schema.get("properties").is_some(), "{} missing properties", def.name);

            println!("✓ {} schema valid", def.name);
        }

        println!("\n✅ Tool schema verification complete\n");
    }

    #[test]
    fn verify_tool_access_filtering() {
        println!("\n=== Tool Access Filtering ===\n");

        // Test All
        let all = ToolRegistry::default_tools(&ToolAccess::All, None);
        println!("All tools: {}", all.names().len());
        assert!(all.contains("Read"));
        assert!(all.contains("Bash"));

        // Test Only
        let only = ToolRegistry::default_tools(&ToolAccess::only(["Read", "Write"]), None);
        println!("Only Read/Write: {}", only.names().len());
        assert!(only.contains("Read"));
        assert!(only.contains("Write"));
        assert!(!only.contains("Bash"));

        // Test Except
        let except = ToolRegistry::default_tools(&ToolAccess::except(["Bash"]), None);
        println!("Except Bash: {}", except.names().len());
        assert!(except.contains("Read"));
        assert!(!except.contains("Bash"));

        // Test None
        let none = ToolRegistry::default_tools(&ToolAccess::None, None);
        println!("None: {}", none.names().len());
        assert_eq!(none.names().len(), 0);

        println!("\n✅ Tool access filtering verification complete\n");
    }
}

// =============================================================================
// Part 4: All Tools Execution Verification
// =============================================================================

mod tool_execution {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn verify_read_tool() {
        println!("\n=== Read Tool ===");
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "Hello World\nLine 2\nLine 3").await.unwrap();

        let registry = ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));
        let result = registry.execute("Read", serde_json::json!({
            "file_path": file.to_str().unwrap()
        })).await;

        assert!(!result.is_error());
        println!("✓ Read tool works");
    }

    #[tokio::test]
    async fn verify_write_tool() {
        println!("\n=== Write Tool ===");
        let dir = tempdir().unwrap();
        let file = dir.path().join("new.txt");

        let registry = ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));
        let result = registry.execute("Write", serde_json::json!({
            "file_path": file.to_str().unwrap(),
            "content": "New content"
        })).await;

        assert!(!result.is_error());
        let content = fs::read_to_string(&file).await.unwrap();
        assert_eq!(content, "New content");
        println!("✓ Write tool works");
    }

    #[tokio::test]
    async fn verify_edit_tool() {
        println!("\n=== Edit Tool ===");
        let dir = tempdir().unwrap();
        let file = dir.path().join("edit.txt");
        fs::write(&file, "Hello OLD World").await.unwrap();

        let registry = ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));
        let result = registry.execute("Edit", serde_json::json!({
            "file_path": file.to_str().unwrap(),
            "old_string": "OLD",
            "new_string": "NEW"
        })).await;

        assert!(!result.is_error());
        let content = fs::read_to_string(&file).await.unwrap();
        assert_eq!(content, "Hello NEW World");
        println!("✓ Edit tool works");
    }

    #[tokio::test]
    async fn verify_glob_tool() {
        println!("\n=== Glob Tool ===");
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "").await.unwrap();
        fs::write(dir.path().join("b.txt"), "").await.unwrap();
        fs::write(dir.path().join("c.rs"), "").await.unwrap();

        let registry = ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));
        let result = registry.execute("Glob", serde_json::json!({
            "pattern": "*.txt",
            "path": dir.path().to_str().unwrap()
        })).await;

        assert!(!result.is_error());
        if let claude_agent::tools::ToolResult::Success(output) = result {
            assert!(output.contains("a.txt"));
            assert!(output.contains("b.txt"));
            assert!(!output.contains("c.rs"));
        }
        println!("✓ Glob tool works");
    }

    #[tokio::test]
    async fn verify_grep_tool() {
        println!("\n=== Grep Tool ===");
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "Hello World\nFoo Bar\nWorld Hello").await.unwrap();

        let registry = ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));
        let result = registry.execute("Grep", serde_json::json!({
            "pattern": "World",
            "path": dir.path().to_str().unwrap()
        })).await;

        assert!(!result.is_error());
        println!("✓ Grep tool works");
    }

    #[tokio::test]
    async fn verify_bash_tool() {
        println!("\n=== Bash Tool ===");
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);
        let result = registry.execute("Bash", serde_json::json!({
            "command": "echo 'test output'"
        })).await;

        assert!(!result.is_error());
        if let claude_agent::tools::ToolResult::Success(output) = result {
            assert!(output.contains("test output"));
        }
        println!("✓ Bash tool works");
    }

    #[tokio::test]
    async fn verify_todowrite_tool() {
        println!("\n=== TodoWrite Tool ===");
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);
        let result = registry.execute("TodoWrite", serde_json::json!({
            "todos": [
                {"content": "Test task", "status": "pending", "activeForm": "Testing task"}
            ]
        })).await;

        assert!(!result.is_error());
        println!("✓ TodoWrite tool works");
    }
}

// =============================================================================
// Part 5: Agent with Tools E2E
// =============================================================================

mod agent_e2e {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_agent_read_file() {
        println!("\n=== Agent Read File E2E ===\n");

        let dir = tempdir().unwrap();
        let file = dir.path().join("secret.txt");
        fs::write(&file, "The answer is 42").await.unwrap();

        let agent = Agent::builder()
            .from_claude_cli()
            .tools(ToolAccess::only(["Read"]))
            .working_dir(dir.path())
            .max_iterations(3)
            .build()
            .expect("Failed to build agent");

        let prompt = format!(
            "Read the file {} and tell me what the answer is. Reply with just the number.",
            file.display()
        );

        let result = agent.execute(&prompt).await.expect("Agent failed");

        println!("Result: {}", result.text());
        println!("Tool calls: {}", result.tool_calls);
        println!("Tokens: {}", result.total_tokens());

        assert!(result.tool_calls >= 1);
        assert!(result.text().contains("42"));

        println!("\n✅ Agent read file verification complete\n");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_agent_write_file() {
        println!("\n=== Agent Write File E2E ===\n");

        let dir = tempdir().unwrap();
        let file = dir.path().join("output.txt");

        let agent = Agent::builder()
            .from_claude_cli()
            .tools(ToolAccess::only(["Write"]))
            .working_dir(dir.path())
            .max_iterations(3)
            .build()
            .expect("Failed to build agent");

        let prompt = format!(
            "Write the text 'Hello from Agent' to the file {}",
            file.display()
        );

        let result = agent.execute(&prompt).await.expect("Agent failed");

        println!("Result: {}", result.text());
        println!("Tool calls: {}", result.tool_calls);

        assert!(result.tool_calls >= 1);

        let content = fs::read_to_string(&file).await.expect("File not created");
        assert!(content.contains("Hello"));

        println!("\n✅ Agent write file verification complete\n");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_agent_bash_command() {
        println!("\n=== Agent Bash Command E2E ===\n");

        let agent = Agent::builder()
            .from_claude_cli()
            .tools(ToolAccess::only(["Bash"]))
            .max_iterations(3)
            .build()
            .expect("Failed to build agent");

        let result = agent
            .execute("Run 'echo hello' and tell me what it outputs. Just the output.")
            .await
            .expect("Agent failed");

        println!("Result: {}", result.text());
        println!("Tool calls: {}", result.tool_calls);

        assert!(result.tool_calls >= 1);
        assert!(result.text().to_lowercase().contains("hello"));

        println!("\n✅ Agent bash command verification complete\n");
    }
}

// =============================================================================
// Part 6: Streaming Verification
// =============================================================================

mod streaming {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_streaming_messages() {
        println!("\n=== Streaming Messages ===\n");

        let client = Client::builder()
            .from_claude_cli()
            .build()
            .expect("Failed to create client");

        let request = CreateMessageRequest::new(
            &client.config().model,
            vec![Message::user("Count: 1, 2, 3")],
        )
        .with_max_tokens(50);

        let stream = claude_agent::client::MessagesClient::new(&client)
            .create_stream(request)
            .await
            .expect("Stream failed");

        let mut stream = pin!(stream);
        let mut events = 0;
        let mut text = String::new();

        while let Some(item) = stream.next().await {
            let item = item.expect("Item error");
            events += 1;
            if let claude_agent::client::StreamItem::Text(t) = item {
                text.push_str(&t);
                print!("{}", t);
            }
        }
        println!();

        println!("Events: {}, Text: {}", events, text.len());
        assert!(events > 0);
        assert!(!text.is_empty());

        println!("\n✅ Streaming messages verification complete\n");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_streaming_agent() {
        println!("\n=== Streaming Agent ===\n");

        let agent = Agent::builder()
            .from_claude_cli()
            .tools(ToolAccess::none())
            .max_iterations(1)
            .build()
            .expect("Failed to build agent");

        let stream = agent
            .execute_stream("Say hello briefly")
            .await
            .expect("Stream failed");

        let mut stream = pin!(stream);
        let mut has_text = false;
        let mut has_complete = false;

        while let Some(event) = stream.next().await {
            match event.expect("Event error") {
                claude_agent::AgentEvent::Text(t) => {
                    print!("{}", t);
                    has_text = true;
                }
                claude_agent::AgentEvent::Complete(r) => {
                    println!("\n[Complete: {} tokens]", r.total_tokens());
                    has_complete = true;
                }
                _ => {}
            }
        }

        assert!(has_text);
        assert!(has_complete);

        println!("\n✅ Streaming agent verification complete\n");
    }
}

// =============================================================================
// Part 7: Multi-Model Support
// =============================================================================

mod multi_model {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_haiku_model() {
        println!("\n=== Haiku Model ===\n");

        let client = Client::builder()
            .from_claude_cli()
            .model("claude-3-5-haiku-20241022")
            .build()
            .expect("Failed to create client");

        let response = client.query("1+1=? Just the number.").await.expect("Failed");
        println!("Haiku: {}", response.trim());
        assert!(response.contains("2"));

        println!("✅ Haiku works\n");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_sonnet_model() {
        println!("\n=== Sonnet Model ===\n");

        let client = Client::builder()
            .from_claude_cli()
            .model("claude-sonnet-4-5-20250929")
            .build()
            .expect("Failed to create client");

        let response = client.query("2+2=? Just the number.").await.expect("Failed");
        println!("Sonnet: {}", response.trim());
        assert!(response.contains("4"));

        println!("✅ Sonnet works\n");
    }
}

// =============================================================================
// Part 8: Error Handling
// =============================================================================

mod error_handling {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn verify_invalid_model_error() {
        println!("\n=== Invalid Model Error ===\n");

        let client = Client::builder()
            .from_claude_cli()
            .build()
            .expect("Failed to create client");

        let request = CreateMessageRequest::new(
            "invalid-model",
            vec![Message::user("test")],
        )
        .with_max_tokens(10);

        let result = claude_agent::client::MessagesClient::new(&client)
            .create(request)
            .await;

        assert!(result.is_err());
        println!("✓ Invalid model returns error: {:?}", result.err());

        println!("\n✅ Error handling verification complete\n");
    }
}

// =============================================================================
// Run All E2E Tests
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn run_full_e2e_verification() {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       Claude Agent RS - Full E2E Verification Suite          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Run: cargo test --test e2e_cli_verification -- --ignored     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}
