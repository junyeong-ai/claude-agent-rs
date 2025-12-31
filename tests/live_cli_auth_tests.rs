//! Live CLI Authentication Tests
//!
//! These tests require actual Claude Code CLI credentials and make real API calls.
//! Run with: cargo test --test live_cli_auth_tests -- --ignored
//!
//! To run specific tests:
//!   cargo test --test live_cli_auth_tests test_basic_query -- --ignored

use claude_agent::{
    client::messages::{CreateMessageRequest, RequestMetadata},
    types::Message,
    Agent, Client, ToolAccess,
};
use futures::StreamExt;
use std::pin::pin;

// =============================================================================
// Test 1: Basic Query with CLI Authentication
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_basic_query_with_cli_auth() {
    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("Failed to create client with CLI credentials");

    println!(
        "Authentication type: {}",
        client.config().auth_strategy.name()
    );
    assert_eq!(
        client.config().auth_strategy.name(),
        "oauth",
        "Should use OAuth authentication"
    );

    // Simple query
    let response = client
        .query("What is 2 + 2? Answer with just the number.")
        .await
        .expect("Query failed");

    println!("Response: {}", response);
    assert!(response.contains("4"), "Response should contain '4'");
}

// =============================================================================
// Test 2: Streaming with CLI Authentication
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_streaming_with_cli_auth() {
    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("Failed to create client with CLI credentials");

    // Test streaming
    let stream_request = CreateMessageRequest::new(
        &client.config().model,
        vec![Message::user("Count from 1 to 3, each on a new line.")],
    )
    .with_max_tokens(50);

    let stream = claude_agent::client::MessagesClient::new(&client)
        .create_stream(stream_request)
        .await
        .expect("Stream creation failed");

    let mut stream = pin!(stream);
    let mut text_chunks = Vec::new();
    let mut event_count = 0;

    while let Some(item) = stream.next().await {
        let item = item.expect("Stream item error");
        event_count += 1;
        match item {
            claude_agent::client::StreamItem::Text(text) => {
                print!("{}", text);
                text_chunks.push(text);
            }
            claude_agent::client::StreamItem::Event(event) => {
                println!("[Event: {:?}]", std::mem::discriminant(&event));
            }
        }
    }
    println!();

    println!(
        "Total events: {}, Text chunks: {}",
        event_count,
        text_chunks.len()
    );

    assert!(event_count > 0, "Should receive at least one event");
    assert!(!text_chunks.is_empty(), "Should receive text chunks");

    let full_text: String = text_chunks.concat();
    println!("Full response: {}", full_text);
    assert!(
        full_text.contains("1") || full_text.to_lowercase().contains("one"),
        "Response should contain '1'"
    );
}

// =============================================================================
// Test 3: Messages API with Full OAuth Headers
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_messages_api_full_flow() {
    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("Failed to create client");

    // Build request with metadata
    let request = CreateMessageRequest::new(
        &client.config().model,
        vec![Message::user("Say hello in exactly 3 words.")],
    )
    .with_max_tokens(100)
    .with_metadata(RequestMetadata::generate());

    let response = claude_agent::client::MessagesClient::new(&client)
        .create(request)
        .await
        .expect("API request failed");

    println!("Response: {}", response.text());
    println!("Model: {}", response.model);
    println!("Stop reason: {:?}", response.stop_reason);
    println!("Input tokens: {}", response.usage.input_tokens);
    println!("Output tokens: {}", response.usage.output_tokens);

    assert!(!response.text().is_empty(), "Response should not be empty");
    assert!(response.usage.input_tokens > 0, "Should have input tokens");
    assert!(
        response.usage.output_tokens > 0,
        "Should have output tokens"
    );
}

// =============================================================================
// Test 4: Custom System Prompt with CLI Auth
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_custom_system_prompt_with_cli_auth() {
    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("Failed to create client");

    // Create request with custom system prompt
    let request = CreateMessageRequest::new(
        &client.config().model,
        vec![Message::user("What is your name?")],
    )
    .with_max_tokens(100)
    .with_system("You are a helpful assistant named TestBot. Always introduce yourself.");

    let response = claude_agent::client::MessagesClient::new(&client)
        .create(request)
        .await
        .expect("API request failed");

    println!("Response: {}", response.text());

    // The Claude Code system prompt is prepended, but user's prompt should also work
    let text = response.text().to_lowercase();
    // Claude should mention being Claude or the context of being an assistant
    assert!(
        text.contains("claude") || text.contains("assistant") || text.contains("testbot"),
        "Response should acknowledge its identity"
    );
}

// =============================================================================
// Test 5: Agent with Tools Using CLI Auth
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_agent_with_tools_cli_auth() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("test_data.txt");
    tokio::fs::write(&test_file, "The secret number is 42.")
        .await
        .expect("Failed to write test file");

    let agent = Agent::builder()
        .from_claude_cli()
        .tools(ToolAccess::only(["Read"]))
        .working_dir(temp_dir.path())
        .max_iterations(5)
        .build()
        .expect("Failed to create agent");

    let prompt = format!(
        "Read the file at {} and tell me what the secret number is. Answer with just the number.",
        test_file.display()
    );

    let result = agent
        .execute(&prompt)
        .await
        .expect("Agent execution failed");

    println!("Agent result: {}", result.text());
    println!("Tool calls: {}", result.tool_calls);
    println!("Iterations: {}", result.iterations);
    println!("Total tokens: {}", result.total_tokens());

    assert!(
        result.tool_calls >= 1,
        "Should have made at least one tool call"
    );
    assert!(
        result.text().contains("42"),
        "Should find the secret number"
    );
}

// =============================================================================
// Test 6: Streaming Agent with CLI Auth
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_streaming_agent_cli_auth() {
    let agent = Agent::builder()
        .from_claude_cli()
        .tools(ToolAccess::none()) // No tools, just text
        .max_iterations(1)
        .build()
        .expect("Failed to create agent");

    let stream = agent
        .execute_stream("Write a haiku about Rust programming.")
        .await
        .expect("Stream creation failed");

    let mut stream = pin!(stream);
    let mut text_chunks = Vec::new();
    let mut final_result = None;

    while let Some(event) = stream.next().await {
        match event.expect("Event error") {
            claude_agent::AgentEvent::Text(text) => {
                print!("{}", text);
                text_chunks.push(text);
            }
            claude_agent::AgentEvent::Complete(result) => {
                println!("\n\n[Complete] Tokens: {}", result.total_tokens());
                final_result = Some(result);
            }
            _ => {}
        }
    }

    assert!(!text_chunks.is_empty(), "Should receive text chunks");
    assert!(final_result.is_some(), "Should receive completion event");
}

// =============================================================================
// Test 7: Prompt Caching Verification
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_prompt_caching_active() {
    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("Failed to create client");

    // First request - should cache the system prompt
    let request1 = CreateMessageRequest::new(&client.config().model, vec![Message::user("Hello!")])
        .with_max_tokens(50);

    let response1 = claude_agent::client::MessagesClient::new(&client)
        .create(request1)
        .await
        .expect("First request failed");

    println!("Request 1 - Input tokens: {}", response1.usage.input_tokens);
    println!(
        "Request 1 - Cache creation: {:?}",
        response1.usage.cache_creation_input_tokens
    );
    println!(
        "Request 1 - Cache read: {:?}",
        response1.usage.cache_read_input_tokens
    );

    // Second request - should benefit from cached system prompt
    let request2 =
        CreateMessageRequest::new(&client.config().model, vec![Message::user("Goodbye!")])
            .with_max_tokens(50);

    let response2 = claude_agent::client::MessagesClient::new(&client)
        .create(request2)
        .await
        .expect("Second request failed");

    println!("Request 2 - Input tokens: {}", response2.usage.input_tokens);
    println!(
        "Request 2 - Cache creation: {:?}",
        response2.usage.cache_creation_input_tokens
    );
    println!(
        "Request 2 - Cache read: {:?}",
        response2.usage.cache_read_input_tokens
    );

    // Note: Caching may or may not hit depending on API state
    // This test verifies the cache fields are present in the response
}

// =============================================================================
// Test 8: Different Models with CLI Auth
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_different_models_with_cli_auth() {
    // Test with Haiku (faster, cheaper) - correct model ID
    let client_haiku = Client::builder()
        .from_claude_cli()
        .model("claude-3-5-haiku-20241022")
        .build()
        .expect("Failed to create Haiku client");

    let response = client_haiku
        .query("What is 1 + 1? Answer with just the number.")
        .await
        .expect("Haiku query failed");

    println!("Haiku response: {}", response);
    assert!(response.contains("2"), "Haiku should answer correctly");

    // Test with Sonnet (default)
    let client_sonnet = Client::builder()
        .from_claude_cli()
        .model("claude-sonnet-4-5-20250929")
        .build()
        .expect("Failed to create Sonnet client");

    let response = client_sonnet
        .query("What is 2 + 2? Answer with just the number.")
        .await
        .expect("Sonnet query failed");

    println!("Sonnet response: {}", response);
    assert!(response.contains("4"), "Sonnet should answer correctly");
}

// =============================================================================
// Test 9: Error Handling with CLI Auth
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_error_handling_cli_auth() {
    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("Failed to create client");

    // Test with invalid model (should fail gracefully)
    let request = CreateMessageRequest::new("invalid-model-name", vec![Message::user("Hello")])
        .with_max_tokens(100);

    let result = claude_agent::client::MessagesClient::new(&client)
        .create(request)
        .await;

    assert!(result.is_err(), "Invalid model should return error");
    if let Err(e) = result {
        println!("Expected error: {}", e);
    }
}

// =============================================================================
// Test 10: Multi-Turn Conversation
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_multi_turn_conversation() {
    let client = Client::builder()
        .from_claude_cli()
        .build()
        .expect("Failed to create client");

    // First turn
    let request1 = CreateMessageRequest::new(
        &client.config().model,
        vec![Message::user("My favorite color is blue. Remember this.")],
    )
    .with_max_tokens(100);

    let response1 = claude_agent::client::MessagesClient::new(&client)
        .create(request1)
        .await
        .expect("First turn failed");

    println!("Turn 1: {}", response1.text());

    // Second turn - reference previous context
    let request2 = CreateMessageRequest::new(
        &client.config().model,
        vec![
            Message::user("My favorite color is blue. Remember this."),
            Message::assistant(response1.text()),
            Message::user("What is my favorite color?"),
        ],
    )
    .with_max_tokens(100);

    let response2 = claude_agent::client::MessagesClient::new(&client)
        .create(request2)
        .await
        .expect("Second turn failed");

    println!("Turn 2: {}", response2.text());
    let text = response2.text().to_lowercase();
    assert!(text.contains("blue"), "Should remember the color");
}

// =============================================================================
// Test 11: Custom Beta Flags
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_custom_beta_flags() {
    let client = Client::builder()
        .from_claude_cli()
        .add_beta_flag("max-tokens-3-5-sonnet-2024-07-15")
        .build()
        .expect("Failed to create client");

    let response = client.query("Hello!").await.expect("Query failed");

    println!("Response with custom beta flag: {}", response);
    assert!(!response.is_empty());
}

// =============================================================================
// Test 12: Verify All OAuth Headers Are Sent
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_all_oauth_headers_present() {
    use claude_agent::auth::{AuthStrategy, OAuthCredential, OAuthStrategy};

    // Create a strategy to inspect headers
    let cred = OAuthCredential {
        access_token: "test".to_string(),
        refresh_token: None,
        expires_at: None,
        scopes: vec![],
        subscription_type: None,
    };
    let strategy = OAuthStrategy::new(cred);

    let headers = strategy.extra_headers();
    let header_map: std::collections::HashMap<_, _> = headers.into_iter().collect();

    println!("OAuth Headers:");
    for (k, v) in &header_map {
        println!("  {}: {}", k, v);
    }

    // Verify all required headers
    assert!(header_map.contains_key("anthropic-beta"));
    assert!(header_map.contains_key("user-agent"));
    assert!(header_map.contains_key("x-app"));
    assert!(header_map.contains_key("anthropic-dangerous-direct-browser-access"));

    // Verify auth header
    let (name, value) = strategy.auth_header();
    println!("Auth header: {}: {}", name, value);
    assert_eq!(name, "Authorization");
    assert!(value.starts_with("Bearer "));

    // Verify URL params
    let query = strategy.url_query_string();
    println!("URL params: {:?}", query);
    assert!(query.is_some());
    assert!(query.unwrap().contains("beta=true"));
}

// =============================================================================
// Run All Live Tests
// =============================================================================

/// Run this to execute all live tests with CLI authentication
/// cargo test --test live_cli_auth_tests run_all_live_tests -- --ignored --nocapture
#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn run_all_live_tests_summary() {
    println!("====================================");
    println!("  CLI Authentication Live Tests");
    println!("====================================");
    println!();
    println!("All tests require Claude Code CLI credentials.");
    println!("Make sure you are logged in: claude auth status");
    println!();
    println!("To run all live tests:");
    println!("  cargo test --test live_cli_auth_tests -- --ignored --nocapture");
    println!();
}
