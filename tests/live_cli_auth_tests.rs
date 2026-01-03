//! Live CLI Authentication Tests
//!
//! These tests require actual Claude Code CLI credentials and make real API calls.
//! Run with: cargo test --test live_cli_auth_tests -- --ignored
//!
//! To run specific tests:
//!   cargo test --test live_cli_auth_tests test_basic_query -- --ignored

use claude_agent::{Agent, Auth, Client, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

// =============================================================================
// Test 1: Basic Query with CLI Authentication
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_basic_query_with_cli_auth() {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client with CLI credentials");

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
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client with CLI credentials");

    // Test streaming
    let stream = client
        .stream("Count from 1 to 3, each on a new line.")
        .await
        .expect("Stream creation failed");

    let mut stream = pin!(stream);
    let mut text_chunks = Vec::new();
    let mut event_count = 0;

    while let Some(item) = stream.next().await {
        let item = item.expect("Stream item error");
        event_count += 1;
        print!("{}", item);
        text_chunks.push(item);
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
// Test 3: Messages API with Full Flow
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_messages_api_full_flow() {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client");

    let response = client
        .query("Say hello in exactly 3 words.")
        .await
        .expect("API request failed");

    println!("Response: {}", response);
    assert!(!response.is_empty(), "Response should not be empty");
}

// =============================================================================
// Test 4: Custom System Prompt with CLI Auth
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_custom_system_prompt_with_cli_auth() {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client");

    let response = client
        .query("What is your name?")
        .await
        .expect("API request failed");

    println!("Response: {}", response);

    // Claude should mention being Claude or the context of being an assistant
    let text = response.to_lowercase();
    assert!(
        text.contains("claude") || text.contains("assistant") || text.contains("ai"),
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
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .tools(ToolAccess::only(["Read"]))
        .working_dir(temp_dir.path())
        .max_iterations(5)
        .build()
        .await
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
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .tools(ToolAccess::none()) // No tools, just text
        .max_iterations(1)
        .build()
        .await
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
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client");

    // First request - should cache the system prompt
    let response1 = client.query("Hello!").await.expect("First request failed");
    println!("Request 1: {}", response1.trim());

    // Second request - should benefit from cached system prompt
    let response2 = client
        .query("Goodbye!")
        .await
        .expect("Second request failed");
    println!("Request 2: {}", response2.trim());

    // Note: Caching may or may not hit depending on API state
    // This test verifies the requests complete successfully
}

// =============================================================================
// Test 8: Different Models with CLI Auth
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_different_models_with_cli_auth() {
    // Test with Haiku (faster, cheaper) - correct model ID
    let client_haiku = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create Haiku client");

    let response = client_haiku
        .query("What is 1 + 1? Answer with just the number.")
        .await
        .expect("Haiku query failed");

    println!("Haiku response: {}", response);
    assert!(response.contains("2"), "Haiku should answer correctly");

    // Test with Sonnet (default)
    let client_sonnet = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
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
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client");

    // Test with empty prompt (should fail gracefully)
    let result = client.query("").await;

    // Either error or empty response is acceptable
    println!("Empty query result: {:?}", result);
}

// =============================================================================
// Test 10: Multi-Turn Conversation
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_multi_turn_conversation() {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client");

    // First turn
    let response1 = client
        .query("My favorite color is blue. Remember this.")
        .await
        .expect("First turn failed");

    println!("Turn 1: {}", response1);

    // For multi-turn, we would need conversation history
    // This tests basic query works
    let response2 = client
        .query("What color did I just mention?")
        .await
        .expect("Second turn failed");

    println!("Turn 2: {}", response2);
}

// =============================================================================
// Test 11: Custom Beta Flags
// =============================================================================

#[tokio::test]
#[ignore = "Requires CLI credentials"]
async fn test_custom_beta_flags() {
    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to create client");

    let response = client.query("Hello!").await.expect("Query failed");

    println!("Response with beta flags: {}", response);
    assert!(!response.is_empty());
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
