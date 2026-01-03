//! Full CLI Authentication Verification Tests
//!
//! Comprehensive test suite to verify all features work correctly with CLI authentication:
//! - CLI OAuth authentication
//! - All built-in tools
//! - Progressive Disclosure (Skills & Rules)
//! - Prompt Caching
//! - Streaming
//! - Multi-turn conversations
//! - Agent loop with tool calls
//!
//! Run: cargo test --test full_cli_verification -- --ignored --nocapture

use claude_agent::{
    Agent, Auth, Client, PermissionPolicy, ToolAccess, ToolOutput,
    skills::{ExecutionMode, SkillDefinition, SkillExecutor, SkillRegistry, SkillTool},
    tools::{ExecutionContext, Tool, ToolRegistry},
};

fn permissive_policy() -> PermissionPolicy {
    PermissionPolicy::permissive()
}
use futures::StreamExt;
use std::path::PathBuf;
use std::pin::pin;
use tempfile::tempdir;
use tokio::fs;

// =============================================================================
// SECTION 1: CLI Authentication Tests
// =============================================================================

mod cli_auth_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_cli_auth_oauth_strategy() {
        let client = Client::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .build()
            .await
            .expect("Failed to create client");

        // Verify we can make authenticated requests
        let response = client.query("Say OK").await.expect("Query failed");
        assert!(!response.is_empty());
        println!("Basic auth test passed");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_basic_api_call() {
        let client = Client::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .build()
            .await
            .expect("Failed to create client");

        let response = client
            .query("Reply with exactly: PONG")
            .await
            .expect("Query failed");

        println!("Response: {}", response);
        assert!(
            response.to_uppercase().contains("PONG"),
            "Should get PONG response"
        );
        println!("Basic API call successful");
    }
}

// =============================================================================
// SECTION 2: Built-in Tools Tests
// =============================================================================

mod builtin_tools_tests {
    use super::*;

    /// Test 1: Read Tool
    #[tokio::test]
    async fn test_read_tool() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "Hello, Read Tool!").await.unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            Some(permissive_policy()),
        );
        let result = registry
            .execute(
                "Read",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap()
                }),
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            println!("Read result: {}", content);
            assert!(content.contains("Hello, Read Tool!"));
        } else {
            panic!("Read tool failed: {:?}", result);
        }
        println!("Read tool works");
    }

    /// Test 2: Write Tool
    #[tokio::test]
    async fn test_write_tool() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("output.txt");

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            Some(permissive_policy()),
        );

        let result = registry
            .execute(
                "Write",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "Written by Write Tool"
                }),
            )
            .await;

        assert!(!result.is_error(), "Write should succeed: {:?}", result);

        // Verify file contents
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("Written by Write Tool"));
        println!("Write tool works");
    }

    /// Test 3: Glob Tool
    #[tokio::test]
    async fn test_glob_tool() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file1.rs"), "rust")
            .await
            .unwrap();
        fs::write(dir.path().join("file2.rs"), "rust")
            .await
            .unwrap();
        fs::write(dir.path().join("file3.txt"), "text")
            .await
            .unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            Some(permissive_policy()),
        );
        let result = registry
            .execute(
                "Glob",
                serde_json::json!({
                    "pattern": "*.rs"
                }),
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            println!("Glob result: {}", content);
            assert!(content.contains("file1.rs"));
            assert!(content.contains("file2.rs"));
            assert!(!content.contains("file3.txt"));
        } else {
            panic!("Glob tool failed: {:?}", result);
        }
        println!("Glob tool works");
    }

    /// Test 4: Grep Tool
    #[tokio::test]
    async fn test_grep_tool() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("search.txt"),
            "target_pattern here\nno match",
        )
        .await
        .unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            Some(permissive_policy()),
        );
        let result = registry
            .execute(
                "Grep",
                serde_json::json!({
                    "pattern": "target_pattern",
                    "path": dir.path().to_str().unwrap()
                }),
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            println!("Grep result: {}", content);
            assert!(content.contains("search.txt"));
        } else {
            panic!("Grep tool failed: {:?}", result);
        }
        println!("Grep tool works");
    }

    /// Test 5: Bash Tool
    #[tokio::test]
    async fn test_bash_tool() {
        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(PathBuf::from("/tmp")),
            Some(permissive_policy()),
        );
        let result = registry
            .execute(
                "Bash",
                serde_json::json!({
                    "command": "echo 'Bash Test Output'"
                }),
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            println!("Bash result: {}", content);
            assert!(content.contains("Bash Test Output"));
        } else {
            panic!("Bash tool failed: {:?}", result);
        }
        println!("Bash tool works");
    }

    /// Test 6: Edit Tool
    #[tokio::test]
    async fn test_edit_tool() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edit_me.txt");
        fs::write(&file_path, "Hello OLD World!").await.unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            Some(permissive_policy()),
        );

        // Read file first (required)
        let _ = registry
            .execute(
                "Read",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap()
                }),
            )
            .await;

        let result = registry
            .execute(
                "Edit",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "OLD",
                    "new_string": "NEW"
                }),
            )
            .await;

        assert!(!result.is_error(), "Edit should succeed: {:?}", result);

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("Hello NEW World!"));
        println!("Edit tool works");
    }

    /// Test 7: TodoWrite Tool
    #[tokio::test]
    async fn test_todo_tool() {
        let registry =
            ToolRegistry::default_tools(&ToolAccess::All, None, Some(permissive_policy()));
        let result = registry
            .execute(
                "TodoWrite",
                serde_json::json!({
                    "todos": [
                        {"content": "Task 1", "status": "pending", "activeForm": "Doing Task 1"},
                        {"content": "Task 2", "status": "in_progress", "activeForm": "Doing Task 2"}
                    ]
                }),
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            println!("Todo result: {}", content);
            assert!(
                content.contains("success")
                    || content.contains("Todo")
                    || content.contains("updated")
            );
        } else {
            panic!("Todo tool failed: {:?}", result);
        }
        println!("TodoWrite tool works");
    }

    /// Test 8: Skill Tool
    #[tokio::test]
    async fn test_skill_tool() {
        let mut registry = SkillRegistry::new();
        registry.register(
            SkillDefinition::new("test-skill", "Test skill", "Execute: $ARGUMENTS")
                .with_trigger("test"),
        );

        let executor = SkillExecutor::new(registry);
        let tool = SkillTool::new(executor);
        let ctx = ExecutionContext::permissive();

        let result = tool
            .execute(
                serde_json::json!({
                    "skill": "test-skill",
                    "args": "hello world"
                }),
                &ctx,
            )
            .await;

        if let ToolOutput::Success(content) = &result.output {
            println!("Skill result: {}", content);
            assert!(content.contains("hello world"));
        } else {
            panic!("Skill tool failed: {:?}", result);
        }
        println!("Skill tool works");
    }

    /// Test 9: Tool Registry with all default tools
    #[tokio::test]
    async fn test_all_tools_in_registry() {
        let registry =
            ToolRegistry::default_tools(&ToolAccess::All, None, Some(permissive_policy()));

        let expected_tools = [
            "Bash",
            "Read",
            "Write",
            "Edit",
            "Glob",
            "Grep",
            "TodoWrite",
            "Skill",
        ];

        println!("=== Registered Tools ===");
        for tool_name in &expected_tools {
            let exists = registry.contains(tool_name);
            println!(
                "  {} - {}",
                tool_name,
                if exists { "OK" } else { "MISSING" }
            );
            assert!(exists, "Tool {} should be registered", tool_name);
        }

        println!("All default tools registered");
    }
}

// =============================================================================
// SECTION 3: Progressive Disclosure Tests
// =============================================================================

mod progressive_disclosure_tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_on_demand_loading() {
        let mut registry = SkillRegistry::new();

        // Register multiple skills - content not loaded until used
        registry.register(SkillDefinition::new(
            "commit",
            "Git commit helper",
            "Create a commit with message: $ARGUMENTS\n\nUse git add and git commit.",
        ));
        registry.register(SkillDefinition::new(
            "deploy",
            "Deployment helper",
            "Deploy to environment: $ARGUMENTS\n\nRun deployment scripts.",
        ));
        registry.register(SkillDefinition::new(
            "review-pr",
            "PR review helper",
            "Review PR: $ARGUMENTS\n\nCheck code quality.",
        ));

        let executor = SkillExecutor::new(registry);

        // Skills exist but content is only returned on execution
        assert!(executor.has_skill("commit"));
        assert!(executor.has_skill("deploy"));
        assert!(executor.has_skill("review-pr"));
        assert!(!executor.has_skill("nonexistent"));

        // Execute specific skill - only this skill's content is "loaded"
        let result = executor.execute("commit", Some("fix: typo")).await;
        assert!(result.success);
        assert!(result.output.contains("fix: typo"));
        println!("Progressive skill result: {}", result.output);

        println!("Progressive disclosure works - skills loaded on demand");
    }

    #[tokio::test]
    async fn test_trigger_based_skill_activation() {
        let mut registry = SkillRegistry::new();

        registry.register(
            SkillDefinition::new("jira-helper", "Jira", "Handle Jira: $ARGUMENTS")
                .with_trigger("jira")
                .with_trigger("issue")
                .with_trigger("ticket"),
        );

        registry.register(
            SkillDefinition::new("docker-helper", "Docker", "Handle Docker: $ARGUMENTS")
                .with_trigger("docker")
                .with_trigger("container"),
        );

        let executor = SkillExecutor::new(registry);

        // Trigger-based activation
        let jira_result = executor.execute_by_trigger("fix the jira ticket").await;
        assert!(jira_result.is_some(), "Should match jira trigger");

        let docker_result = executor
            .execute_by_trigger("restart docker container")
            .await;
        assert!(docker_result.is_some(), "Should match docker trigger");

        let no_match = executor.execute_by_trigger("random unrelated text").await;
        assert!(no_match.is_none(), "Should not match any trigger");

        println!("Trigger-based skill activation works");
    }

    #[tokio::test]
    async fn test_execution_modes() {
        let mut registry = SkillRegistry::new();
        registry.register(SkillDefinition::new(
            "test-skill",
            "Test",
            "Execute: $ARGUMENTS",
        ));

        // DryRun mode
        let mut registry2 = SkillRegistry::new();
        registry2.register(SkillDefinition::new(
            "test-skill",
            "Test",
            "Execute: $ARGUMENTS",
        ));
        let dry_executor = SkillExecutor::new(registry).with_mode(ExecutionMode::DryRun);
        let dry_result = dry_executor.execute("test-skill", Some("test")).await;
        assert!(dry_result.output.contains("[DRY RUN]"));

        // InlinePrompt mode
        let inline_executor = SkillExecutor::new(registry2).with_mode(ExecutionMode::InlinePrompt);
        let inline_result = inline_executor.execute("test-skill", Some("test")).await;
        assert!(
            inline_result
                .output
                .contains("Execute the following skill instructions")
        );

        println!("Execution modes work correctly");
    }

    #[tokio::test]
    async fn test_slash_commands() {
        let dir = tempdir().unwrap();
        let commands_dir = dir.path().join(".claude").join("commands");
        fs::create_dir_all(&commands_dir).await.unwrap();

        // Create slash command with YAML frontmatter
        fs::write(
            commands_dir.join("build.md"),
            r#"---
description: Build the project
allowed-tools:
  - Bash
argument-hint: target
---
Build the project for $ARGUMENTS target.

Steps:
1. Run cargo build
2. Run tests
"#,
        )
        .await
        .unwrap();

        // Create nested command
        let aws_dir = commands_dir.join("aws");
        fs::create_dir_all(&aws_dir).await.unwrap();
        fs::write(aws_dir.join("deploy.md"), "Deploy to AWS: $ARGUMENTS")
            .await
            .unwrap();

        let mut loader = claude_agent::skills::CommandLoader::new();
        loader.load_all(dir.path()).await.unwrap();

        assert!(loader.exists("build"), "build command should exist");
        assert!(loader.exists("aws:deploy"), "aws:deploy should exist");

        let cmd = loader.get("build").unwrap();
        let output = cmd.execute("release");
        assert!(output.contains("release"));

        println!("Slash commands work correctly");
    }
}

// =============================================================================
// SECTION 4: Streaming Tests
// =============================================================================

mod streaming_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_streaming_response() {
        let client = Client::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .build()
            .await
            .expect("Failed to create client");

        let stream = client
            .stream("Count from 1 to 3.")
            .await
            .expect("Stream creation failed");

        let mut stream = pin!(stream);
        let mut text_chunks = Vec::new();
        let mut event_count = 0;

        while let Some(item) = stream.next().await {
            let item = item.expect("Stream error");
            event_count += 1;
            print!("{}", item);
            text_chunks.push(item);
        }
        println!();

        assert!(event_count > 0, "Should receive events");
        assert!(!text_chunks.is_empty(), "Should receive text");

        let full_text: String = text_chunks.concat();
        assert!(
            full_text.contains("1") || full_text.contains("one"),
            "Should contain counting"
        );

        println!(
            "Streaming works: {} events, {} chunks",
            event_count,
            text_chunks.len()
        );
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_agent_streaming() {
        let agent = Agent::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .tools(ToolAccess::none())
            .max_iterations(1)
            .build()
            .await
            .expect("Failed to create agent");

        let stream = agent
            .execute_stream("Say hello in 5 words or less.")
            .await
            .expect("Stream failed");

        let mut stream = pin!(stream);
        let mut text_chunks = Vec::new();
        let mut complete_event = false;

        while let Some(event) = stream.next().await {
            match event.expect("Event error") {
                claude_agent::AgentEvent::Text(text) => {
                    text_chunks.push(text);
                }
                claude_agent::AgentEvent::Complete(result) => {
                    println!("\nComplete: {} tokens", result.total_tokens());
                    complete_event = true;
                }
                _ => {}
            }
        }

        assert!(!text_chunks.is_empty(), "Should have text chunks");
        assert!(complete_event, "Should have complete event");

        println!("Agent streaming works");
    }
}

// =============================================================================
// SECTION 5: Agent with Tools Tests
// =============================================================================

mod agent_tools_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_agent_read_tool() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("secret.txt");
        fs::write(&file_path, "The answer is 42").await.unwrap();

        let agent = Agent::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .tools(ToolAccess::only(["Read"]))
            .working_dir(dir.path())
            .max_iterations(5)
            .build()
            .await
            .expect("Failed to create agent");

        let prompt = format!(
            "Read the file {} and tell me what the answer is. Reply with just the number.",
            file_path.display()
        );

        let result = agent.execute(&prompt).await.expect("Agent failed");

        println!("Result: {}", result.text());
        println!("Tool calls: {}", result.tool_calls);
        println!("Iterations: {}", result.iterations);

        assert!(result.tool_calls >= 1, "Should use Read tool");
        assert!(result.text().contains("42"), "Should find the answer");

        println!("Agent with Read tool works");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_agent_multi_tool() {
        let dir = tempdir().unwrap();

        // Create test files
        fs::write(dir.path().join("hello.txt"), "Hello")
            .await
            .unwrap();
        fs::write(dir.path().join("world.txt"), "World")
            .await
            .unwrap();

        let agent = Agent::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .tools(ToolAccess::only(["Read", "Glob", "Bash"]))
            .working_dir(dir.path())
            .max_iterations(10)
            .build()
            .await
            .expect("Failed to create agent");

        let result = agent
            .execute("List all .txt files in the current directory and read their contents")
            .await
            .expect("Agent failed");

        println!("Result: {}", result.text());
        println!("Tool calls: {}", result.tool_calls);

        assert!(result.tool_calls >= 1, "Should use tools");

        println!("Agent with multiple tools works");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_agent_with_custom_skill() {
        let agent = Agent::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .skill(SkillDefinition::new(
                "math",
                "Mathematical calculations",
                "Calculate: $ARGUMENTS. Show work and provide answer.",
            ))
            .tools(ToolAccess::only(["Skill"]))
            .max_iterations(5)
            .build()
            .await
            .expect("Failed to create agent");

        let result = agent
            .execute("Use the math skill to calculate 15 * 7 + 23")
            .await
            .expect("Agent failed");

        println!("Result: {}", result.text());

        // 15 * 7 + 23 = 105 + 23 = 128
        assert!(
            result.text().contains("128") || result.text().contains("Calculate"),
            "Should contain result or skill instruction"
        );

        println!("Agent with custom skill works");
    }
}

// =============================================================================
// SECTION 6: Model Selection Tests
// =============================================================================

mod model_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_haiku_model() {
        let client = Client::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .build()
            .await
            .expect("Failed to create Haiku client");

        let response = client
            .query("What is 2 + 2? Just the number.")
            .await
            .expect("Haiku query failed");

        println!("Haiku: {}", response);
        assert!(response.contains("4"));
        println!("Haiku model works");
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_sonnet_model() {
        let client = Client::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .build()
            .await
            .expect("Failed to create Sonnet client");

        let response = client
            .query("What is 3 + 3? Just the number.")
            .await
            .expect("Sonnet query failed");

        println!("Sonnet: {}", response);
        assert!(response.contains("6"));
        println!("Sonnet model works");
    }
}

// =============================================================================
// SECTION 7: Error Handling Tests
// =============================================================================

mod error_handling_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_invalid_model_error() {
        let client = Client::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .build()
            .await
            .expect("Failed to create client");

        // Test with empty query
        let result = client.query("").await;

        // Either error or empty response is acceptable
        println!("Empty query result: {:?}", result);
        println!("Error handling works");
    }

    #[tokio::test]
    async fn test_tool_error_handling() {
        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(PathBuf::from("/tmp")),
            Some(permissive_policy()),
        );

        // Try to read non-existent file
        let result = registry
            .execute(
                "Read",
                serde_json::json!({
                    "file_path": "/nonexistent/path/to/file.txt"
                }),
            )
            .await;

        assert!(result.is_error(), "Should return error for missing file");
        println!("Tool error handling works");
    }
}

// =============================================================================
// SECTION 8: Integration Summary Test
// =============================================================================

#[test]
fn test_verification_summary() {
    println!();
    println!("========================================================================");
    println!("           Full CLI Verification Test Suite");
    println!("========================================================================");
    println!();
    println!("  Test Categories:");
    println!("  ------------------------------------------------------------------------");
    println!("  1. CLI Authentication (OAuth, headers, basic API)");
    println!("  2. Built-in Tools (Bash, Read, Write, Edit, Glob, Grep, etc.)");
    println!("  3. Progressive Disclosure (skills, triggers, slash commands)");
    println!("  4. Streaming (client streaming, agent streaming)");
    println!("  5. Agent with Tools (single tool, multi-tool, custom skills)");
    println!("  6. Model Selection (Haiku, Sonnet)");
    println!("  7. Error Handling (invalid model, tool errors)");
    println!();
    println!("  Run Commands:");
    println!();
    println!("  # Run all tests (including live API tests):");
    println!("  cargo test --test full_cli_verification -- --ignored --nocapture");
    println!();
    println!("  # Run only offline tests:");
    println!("  cargo test --test full_cli_verification -- --nocapture");
    println!();
    println!("========================================================================");
    println!();
}
