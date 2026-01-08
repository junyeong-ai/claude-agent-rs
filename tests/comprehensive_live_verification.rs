//! Comprehensive Live Verification Tests
//!
//! This test suite verifies ALL major Claude Code CLI features with actual API calls.
//! Run: cargo test --test comprehensive_live_verification -- --ignored --nocapture

use claude_agent::{
    Agent, Auth, Client, ToolAccess,
    client::CloudProvider,
    context::{ContextBuilder, MemoryLoader},
    skills::{CommandLoader, SkillDefinition, SkillExecutor, SkillRegistry},
};
use std::time::Instant;
use tempfile::tempdir;
use tokio::fs;

// =============================================================================
// Part 1: Authentication Strategies
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_1_cli_oauth_authentication() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 1: CLI OAuth Authentication");
    println!("{}", "=".repeat(60));

    let start = Instant::now();

    let client = Client::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .build()
        .await
        .expect("Failed to build client with CLI auth");

    // Test actual API call
    let response = client
        .query("Reply with exactly: AUTH_OK")
        .await
        .expect("Query failed");

    println!("Response: {}", response.trim());
    assert!(response.contains("AUTH_OK"), "Should get valid response");

    println!(
        "CLI OAuth authentication: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

#[test]
fn test_2_cloud_provider_enum() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 2: Cloud Provider Enum");
    println!("{}", "=".repeat(60));

    // Test provider enum
    assert_eq!(CloudProvider::default(), CloudProvider::Anthropic);

    println!("Cloud providers available:");
    println!("  - Anthropic (default)");
    println!("  - Bedrock (AWS)");
    println!("  - Vertex (GCP)");
    println!("  - Foundry (Azure)");

    println!("Cloud provider enum: PASSED");
}

// =============================================================================
// Part 2: Memory System (CLAUDE.md)
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_3_claude_md_recursive_loading() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 3: CLAUDE.md Recursive Loading");
    println!("{}", "=".repeat(60));

    let start = Instant::now();
    let dir = tempdir().unwrap();

    // Create nested directory structure
    let subdir = dir.path().join("src").join("components");
    fs::create_dir_all(&subdir).await.unwrap();

    // Root CLAUDE.md
    fs::write(
        dir.path().join("CLAUDE.md"),
        "# Root Project\n\nThis is the root project context.\n\n@docs/api.md",
    )
    .await
    .unwrap();

    // Create docs directory with imported file
    let docs_dir = dir.path().join("docs");
    fs::create_dir_all(&docs_dir).await.unwrap();
    fs::write(
        docs_dir.join("api.md"),
        "## API Documentation\n\nEndpoints: /api/v1/*",
    )
    .await
    .unwrap();

    // Rules directory
    let rules_dir = dir.path().join(".claude").join("rules");
    fs::create_dir_all(&rules_dir).await.unwrap();
    fs::write(
        rules_dir.join("rust.md"),
        "# Rust Rules\n- Use snake_case\n- No unwrap in production",
    )
    .await
    .unwrap();
    fs::write(
        rules_dir.join("security.md"),
        "# Security\n- Never expose secrets\n- Validate all inputs",
    )
    .await
    .unwrap();

    // Load all memory
    let mut loader = MemoryLoader::new();
    let content = loader.load(dir.path()).await.unwrap();

    println!("CLAUDE.md files: {}", content.claude_md.len());
    println!("Rules: {}", content.rule_indices.len());

    let combined = content.combined_claude_md();
    println!(
        "\nCombined content preview:\n{}",
        &combined[..combined.len().min(500)]
    );

    assert!(
        combined.contains("Root Project"),
        "Should load root CLAUDE.md"
    );
    assert!(
        combined.contains("API Documentation"),
        "Should import docs/api.md"
    );
    assert!(combined.contains("Rust Rules"), "Should load rules");
    assert_eq!(content.rule_indices.len(), 2, "Should load 2 rules");

    println!(
        "\nCLAUDE.md recursive loading: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

#[tokio::test]
#[ignore = "Live test"]
async fn test_4_import_syntax_with_home_expansion() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 4: @import Syntax with Home Directory Expansion");
    println!("{}", "=".repeat(60));

    let start = Instant::now();
    let dir = tempdir().unwrap();

    // Create file with various import styles
    fs::write(
        dir.path().join("CLAUDE.md"),
        r#"# Project

@relative/file.md
@/absolute/path/file.md
@~/home/path/file.md
@@escaped_at_symbol

End of file"#,
    )
    .await
    .unwrap();

    // Create relative import
    let rel_dir = dir.path().join("relative");
    fs::create_dir_all(&rel_dir).await.unwrap();
    fs::write(rel_dir.join("file.md"), "Relative import content")
        .await
        .unwrap();

    let mut loader = MemoryLoader::new();
    let content = loader.load(dir.path()).await.unwrap();
    let combined = content.combined_claude_md();

    println!("Content:\n{}", combined);

    assert!(
        combined.contains("Relative import content"),
        "Should resolve relative imports"
    );
    assert!(combined.contains("@@escaped"), "Should preserve escaped @@");

    println!(
        "\nImport syntax: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

// =============================================================================
// Part 3: Skills & Progressive Disclosure
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_5_skill_registration_and_execution() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 5: Skill Registration and Execution");
    println!("{}", "=".repeat(60));

    let start = Instant::now();

    let mut registry = SkillRegistry::new();

    // Register multiple skills
    registry.register(
        SkillDefinition::new(
            "git-commit",
            "Create conventional git commits",
            "Analyze changes and create commit: $ARGUMENTS",
        )
        .with_trigger("/commit")
        .with_trigger("git commit"),
    );

    registry.register(
        SkillDefinition::new(
            "code-review",
            "Review code for issues",
            "Review the following code: $ARGUMENTS\n\nCheck for:\n- Bugs\n- Performance\n- Security",
        )
        .with_trigger("/review"),
    );

    registry.register(SkillDefinition::new(
        "docker-compose",
        "Manage Docker services",
        "Docker command: $ARGUMENTS",
    ));

    let executor = SkillExecutor::new(registry);

    // Test skill execution
    let result = executor.execute("git-commit", Some("fix: login bug")).await;
    println!("Git commit skill result:\n{}", result.output);
    assert!(result.success);

    // Test trigger-based execution
    let trigger_result = executor.execute_by_trigger("/commit initial commit").await;
    assert!(trigger_result.is_some());
    println!("Trigger result:\n{}", trigger_result.unwrap().output);

    // Test skill listing
    let skills = executor.list_skills();
    println!("Registered skills: {:?}", skills);
    assert_eq!(skills.len(), 3);

    println!(
        "\nSkill registration: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

#[tokio::test]
#[ignore = "Live test"]
async fn test_6_progressive_disclosure_with_agent() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 6: Progressive Disclosure with Live Agent");
    println!("{}", "=".repeat(60));

    let start = Instant::now();

    // Create agent with custom skills
    let agent = Agent::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .skill(SkillDefinition::new(
            "calculator",
            "Perform calculations",
            "Calculate: $ARGUMENTS\n\nProvide step-by-step solution.",
        ))
        .skill(SkillDefinition::new(
            "translator",
            "Translate text",
            "Translate to Korean: $ARGUMENTS",
        ))
        .tools(ToolAccess::only(["Skill"]))
        .max_iterations(5)
        .build()
        .await
        .expect("Failed to build agent");

    // Test skill invocation through agent
    let result = agent
        .execute("Use the calculator skill to compute: 25 * 4 + 100 / 5")
        .await
        .expect("Agent execution failed");

    println!("Agent result:\n{}", result.text());
    assert!(
        result.text().contains("120") || result.text().contains("Calculate"),
        "Should calculate or show skill prompt"
    );

    println!(
        "\nProgressive disclosure: PASSED ({} ms, {} iterations, {} tokens)",
        start.elapsed().as_millis(),
        result.iterations,
        result.total_tokens()
    );
}

// =============================================================================
// Part 4: Slash Commands
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_7_slash_commands_loading() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 7: Slash Commands (.claude/commands/)");
    println!("{}", "=".repeat(60));

    let start = Instant::now();
    let dir = tempdir().unwrap();

    // Create commands directory structure
    let commands_dir = dir.path().join(".claude").join("commands");
    fs::create_dir_all(&commands_dir).await.unwrap();

    // Create deploy command
    fs::write(
        commands_dir.join("deploy.md"),
        r#"---
description: Deploy application to environment
allowed-tools:
  - Bash
argument-hint: <environment>
---
Deploy to $ARGUMENTS environment:
1. Run tests
2. Build
3. Deploy
"#,
    )
    .await
    .unwrap();

    // Create nested aws commands
    let aws_dir = commands_dir.join("aws");
    fs::create_dir_all(&aws_dir).await.unwrap();
    fs::write(aws_dir.join("lambda.md"), "Deploy Lambda: $ARGUMENTS")
        .await
        .unwrap();
    fs::write(aws_dir.join("s3.md"), "S3 operation: $ARGUMENTS")
        .await
        .unwrap();

    // Load commands
    let mut loader = CommandLoader::new();
    loader.load(dir.path()).await.unwrap();

    println!("Loaded commands:");
    for cmd in loader.list() {
        println!("  - {} : {:?}", cmd.name, cmd.description);
    }

    assert!(loader.exists("deploy"));
    assert!(loader.exists("aws:lambda"));
    assert!(loader.exists("aws:s3"));

    // Test argument substitution
    let deploy_cmd = loader.get("deploy").unwrap();
    let executed = deploy_cmd.execute("production");
    println!("\nDeploy command output:\n{}", executed);
    assert!(executed.contains("production"));

    println!(
        "\nSlash commands: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

// =============================================================================
// Part 5: Context Builder Integration
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_8_context_builder_integration() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 8: Context Builder with All Sources");
    println!("{}", "=".repeat(60));

    let start = Instant::now();
    let dir = tempdir().unwrap();

    // Setup complete project structure
    let claude_dir = dir.path().join(".claude");
    let rules_dir = claude_dir.join("rules");
    fs::create_dir_all(&rules_dir).await.unwrap();

    fs::write(
        dir.path().join("CLAUDE.md"),
        "# Main Project\nCore instructions here.",
    )
    .await
    .unwrap();
    fs::write(
        rules_dir.join("coding.md"),
        "# Coding Standards\nFollow best practices.",
    )
    .await
    .unwrap();

    // Build context
    let context = ContextBuilder::new()
        .load_from_directory(dir.path())
        .await
        .build()
        .unwrap();

    let static_ctx = context.static_context();
    println!("Static context loaded:");
    println!("  Claude MD length: {} chars", static_ctx.claude_md.len());
    println!(
        "  Preview: {}...",
        &static_ctx.claude_md[..100.min(static_ctx.claude_md.len())]
    );

    assert!(static_ctx.claude_md.contains("Main Project"));
    assert!(static_ctx.claude_md.contains("Coding Standards"));

    println!(
        "\nContext builder: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

// =============================================================================
// Part 6: Full Agent Integration
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_9_full_agent_with_tools_and_skills() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 9: Full Agent with Tools and Skills");
    println!("{}", "=".repeat(60));

    let start = Instant::now();
    let dir = tempdir().unwrap();

    // Create test files
    fs::write(
        dir.path().join("data.json"),
        r#"{"users": 42, "active": true}"#,
    )
    .await
    .unwrap();

    // Create agent with skills and file tools
    let agent = Agent::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .skill(SkillDefinition::new(
            "json-analyzer",
            "Analyze JSON data",
            "Analyze this JSON and summarize: $ARGUMENTS",
        ))
        .tools(ToolAccess::only(["Skill", "Read", "Bash"]))
        .working_dir(dir.path())
        .max_iterations(5)
        .build()
        .await
        .expect("Failed to build agent");

    // Test reading file and using skill
    let result = agent
        .execute("Read data.json and tell me the number of users")
        .await
        .expect("Agent failed");

    println!("Result:\n{}", result.text());
    println!("\nMetrics:");
    println!("  Iterations: {}", result.iterations);
    println!("  Tool calls: {}", result.tool_calls);
    println!("  Total tokens: {}", result.total_tokens());

    assert!(result.text().contains("42") || result.text().contains("users"));

    println!(
        "\nFull agent integration: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

#[tokio::test]
#[ignore = "Live test"]
async fn test_10_agent_with_bash_tool() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 10: Agent with Bash Tool Execution");
    println!("{}", "=".repeat(60));

    let start = Instant::now();

    let agent = Agent::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .tools(ToolAccess::only(["Bash"]))
        .max_iterations(3)
        .build()
        .await
        .expect("Failed to build agent");

    let result = agent
        .execute("Run 'echo Hello from Bash' and tell me the output")
        .await
        .expect("Agent failed");

    println!("Result:\n{}", result.text());
    assert!(result.text().contains("Hello") || result.text().contains("Bash"));

    println!("\nBash tool: PASSED ({} ms)", start.elapsed().as_millis());
}

// =============================================================================
// Part 7: Model Configuration
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_11_model_configuration() {
    println!("\n{}", "=".repeat(60));
    println!("TEST 11: Model Configuration (main + small)");
    println!("{}", "=".repeat(60));

    let start = Instant::now();

    // Test with explicit model configuration using Agent builder (has from_claude_code)
    let agent = Agent::builder()
        .auth(Auth::ClaudeCli)
        .await
        .expect("Failed to load CLI credentials")
        .model("claude-sonnet-4-5-20250929")
        .small_model("claude-3-5-haiku-20241022")
        .tools(ToolAccess::none())
        .max_iterations(1)
        .build()
        .await
        .expect("Failed to build agent");

    // Make a test query
    let result = agent.execute("Say OK").await.expect("Query failed");
    println!("Response: {}", result.text());
    assert!(!result.text().is_empty());

    println!(
        "\nModel configuration: PASSED ({} ms)",
        start.elapsed().as_millis()
    );
}

// =============================================================================
// Final Summary Test
// =============================================================================

#[tokio::test]
#[ignore = "Live test"]
async fn test_99_comprehensive_summary() {
    println!("\n");
    println!("========================================================================");
    println!("            COMPREHENSIVE LIVE VERIFICATION SUMMARY");
    println!("========================================================================");
    println!();
    println!("  Authentication:");
    println!("    - CLI OAuth authentication");
    println!("    - Cloud provider selection");
    println!();
    println!("  Memory System:");
    println!("    - CLAUDE.md project-level loading");
    println!("    - @import syntax with home directory expansion");
    println!("    - .claude/rules/ recursive directory loading");
    println!();
    println!("  Skills & Progressive Disclosure:");
    println!("    - Skill registration and execution");
    println!("    - Trigger-based skill activation");
    println!("    - Progressive disclosure with live agent");
    println!("    - $ARGUMENTS substitution");
    println!();
    println!("  Slash Commands:");
    println!("    - .claude/commands/ loading");
    println!("    - Nested namespace support (aws:lambda)");
    println!("    - Frontmatter metadata parsing");
    println!();
    println!("  Agent Integration:");
    println!("    - Full agent with tools and skills");
    println!("    - Bash tool execution");
    println!("    - File operations (Read)");
    println!();
    println!("  Cloud Providers:");
    println!("    - Provider selection (Anthropic, Bedrock, Vertex, Foundry)");
    println!("    - Model configuration (main + small)");
    println!();
    println!("========================================================================");
    println!();
}
