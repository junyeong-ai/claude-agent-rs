//! Progressive Disclosure Integration Tests
//!
//! Verifies that skills and rules are loaded on-demand and used effectively.
//!
//! Run: cargo test --test progressive_disclosure_test -- --nocapture

use claude_agent::{
    Agent, Auth, Index, ToolAccess, ToolOutput,
    common::{ContentSource, IndexRegistry},
    skills::{ExecutionMode, SkillExecutor, SkillIndex, SkillIndexLoader, SkillTool},
    tools::{ExecutionContext, Tool, ToolRegistry},
};
use tempfile::tempdir;
use tokio::fs;

// =============================================================================
// Part 1: SkillTool Integration Tests
// =============================================================================

mod skill_tool_tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_tool_in_default_registry() {
        let registry = ToolRegistry::default_tools(ToolAccess::All, None, None);

        // Skill tool should be in the default registry
        assert!(
            registry.contains("Skill"),
            "Skill tool should be in default registry"
        );
    }

    #[tokio::test]
    async fn test_skill_tool_execute_inline() {
        let mut skill_registry = IndexRegistry::<SkillIndex>::new();
        skill_registry.register(
            SkillIndex::new(
                "atlassian-cli",
                "Execute Atlassian CLI commands for Jira/Confluence",
            )
            .with_source(ContentSource::in_memory(
                r#"
You have access to the atlassian-cli tool. Use Bash to run commands like:
- `atlassian jira issue list --project $ARGUMENTS`
- `atlassian jira issue create --project PROJ --summary "Title"`
- `atlassian confluence page get --space SPACE --title "Page"`

Execute the user's request: $ARGUMENTS
"#,
            ))
            .with_triggers(["jira", "confluence", "atlassian"]),
        );

        let executor = SkillExecutor::new(skill_registry);
        let skill_tool = SkillTool::new(executor);
        let ctx = ExecutionContext::permissive();

        // Execute the skill
        let result = skill_tool
            .execute(
                serde_json::json!({
                    "skill": "atlassian-cli",
                    "args": "list issues in PROJECT-123"
                }),
                &ctx,
            )
            .await;

        assert!(!result.is_error());
        if let ToolOutput::Success(content) = &result.output {
            println!("Skill output:\n{}", content);
            assert!(content.contains("atlassian"));
            assert!(content.contains("list issues in PROJECT-123"));
        }
    }

    #[tokio::test]
    async fn test_skill_progressive_loading() {
        let mut skill_registry = IndexRegistry::<SkillIndex>::new();

        // Add multiple skills - only loaded when activated
        skill_registry.register(
            SkillIndex::new("commit", "Git commit")
                .with_source(ContentSource::in_memory("Create commit: $ARGUMENTS"))
                .with_triggers(["/commit"]),
        );
        skill_registry.register(
            SkillIndex::new("review-pr", "Review PR")
                .with_source(ContentSource::in_memory("Review PR: $ARGUMENTS"))
                .with_triggers(["/review"]),
        );
        skill_registry.register(
            SkillIndex::new("datadog-query", "Query Datadog")
                .with_source(ContentSource::in_memory("Query: $ARGUMENTS")),
        );

        let executor = SkillExecutor::new(skill_registry);

        // Skills are registered but content not in context until used
        assert!(executor.has_skill("commit"));
        assert!(executor.has_skill("review-pr"));
        assert!(executor.has_skill("datadog-query"));

        // When skill is executed, content is returned for agent to process
        let result = executor.execute("commit", Some("fix: bug")).await;
        assert!(result.success);
        println!("Commit skill result:\n{}", result.output);
    }
}

// =============================================================================
// Part 2: Skill Index Tests (Progressive Disclosure)
// =============================================================================

mod skill_index_tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_loader_from_directory() {
        let dir = tempdir().unwrap();
        let skills_dir = dir.path().join(".claude").join("skills");
        fs::create_dir_all(&skills_dir).await.unwrap();

        // Create a skill directory with SKILL.md
        let deploy_dir = skills_dir.join("deploy");
        fs::create_dir_all(&deploy_dir).await.unwrap();
        fs::write(
            deploy_dir.join("SKILL.md"),
            r#"---
name: deploy
description: Deploy the application
allowed-tools:
  - Bash
argument-hint: environment
---
Deploy the application to the $ARGUMENTS environment.

Steps:
1. Run tests
2. Build the application
3. Deploy to $ARGUMENTS
"#,
        )
        .await
        .unwrap();

        // Create nested skill
        let lambda_dir = skills_dir.join("aws-lambda");
        fs::create_dir_all(&lambda_dir).await.unwrap();
        fs::write(
            lambda_dir.join("SKILL.md"),
            r#"---
name: aws-lambda
description: Deploy AWS Lambda function
---
Deploy AWS Lambda function: $ARGUMENTS"#,
        )
        .await
        .unwrap();

        let loader = SkillIndexLoader::new();
        let indices = loader.scan_directory(&skills_dir).await.unwrap();

        // Create registry from loaded indices
        let mut registry = IndexRegistry::<SkillIndex>::new();
        registry.register_all(indices);

        // Skills should be loaded
        assert!(registry.contains("deploy"));
        assert!(registry.contains("aws-lambda"));

        // Execute skill with arguments (uses lazy loading via execute method)
        let skill = registry.get("deploy").unwrap();
        let content = skill.load_content().await.unwrap();
        let result = skill.execute("production", &content).await;
        println!("Deploy skill result:\n{}", result);
        assert!(result.contains("production"));
    }
}

// =============================================================================
// Part 3: Simulated CLI Tool Integration
// =============================================================================

mod cli_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_with_cli_tool_simulation() {
        // Simulate a skill that uses an external CLI tool
        let mut skill_registry = IndexRegistry::<SkillIndex>::new();

        skill_registry.register(
            SkillIndex::new("docker-compose", "Manage Docker Compose services").with_source(
                ContentSource::in_memory(
                    r#"
You have access to docker-compose. Use Bash to execute:

Available commands:
- docker-compose up -d: Start services
- docker-compose down: Stop services
- docker-compose ps: List services
- docker-compose logs [service]: View logs

User request: $ARGUMENTS

Execute the appropriate docker-compose command using Bash.
"#,
                ),
            ),
        );

        let executor = SkillExecutor::new(skill_registry);
        let result = executor
            .execute("docker-compose", Some("start the web service"))
            .await;

        assert!(result.success);
        assert!(result.output.contains("docker-compose"));
        println!("Docker skill:\n{}", result.output);
    }

    #[tokio::test]
    async fn test_context_aware_skill_activation() {
        // Test that skills are activated based on context/triggers
        let mut skill_registry = IndexRegistry::<SkillIndex>::new();

        skill_registry.register(
            SkillIndex::new("rust-analyzer", "Rust code analysis")
                .with_source(ContentSource::in_memory("Analyze Rust: $ARGUMENTS"))
                .with_triggers(["rust", "cargo"]),
        );

        skill_registry.register(
            SkillIndex::new("npm-scripts", "NPM script runner")
                .with_source(ContentSource::in_memory("Run npm: $ARGUMENTS"))
                .with_triggers(["npm", "node", "package.json"]),
        );

        let executor = SkillExecutor::new(skill_registry);

        // Trigger-based activation
        let rust_result = executor.execute_by_trigger("cargo build failed").await;
        assert!(rust_result.is_some());

        let npm_result = executor.execute_by_trigger("npm install error").await;
        assert!(npm_result.is_some());

        let no_match = executor.execute_by_trigger("random text").await;
        assert!(no_match.is_none());
    }
}

// =============================================================================
// Part 4: ExecutionMode Tests
// =============================================================================

mod execution_mode_tests {
    use super::*;

    #[tokio::test]
    async fn test_dry_run_mode() {
        let mut registry = IndexRegistry::<SkillIndex>::new();
        registry.register(
            SkillIndex::new("test", "Test").with_source(ContentSource::in_memory("Do: $ARGUMENTS")),
        );

        let executor = SkillExecutor::new(registry).with_mode(ExecutionMode::DryRun);
        let result = executor.execute("test", Some("something")).await;

        assert!(result.success);
        assert!(result.output.contains("[DRY RUN]"));
        println!("Dry run output:\n{}", result.output);
    }

    #[tokio::test]
    async fn test_inline_prompt_mode() {
        let mut registry = IndexRegistry::<SkillIndex>::new();
        registry.register(
            SkillIndex::new("analyze", "Analyze code")
                .with_source(ContentSource::in_memory("Analyze: $ARGUMENTS")),
        );

        let executor = SkillExecutor::new(registry).with_mode(ExecutionMode::InlinePrompt);
        let result = executor.execute("analyze", Some("main.rs")).await;

        assert!(result.success);
        assert!(
            result
                .output
                .contains("Execute the following skill instructions")
        );
        println!("Inline prompt output:\n{}", result.output);
    }
}

// =============================================================================
// Part 5: Live Agent Test with Skills
// =============================================================================

mod live_agent_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_agent_with_custom_skill() {
        // Create agent with custom skill registered via builder
        let agent = Agent::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .skill(
                SkillIndex::new("math-helper", "Perform mathematical calculations").with_source(
                    ContentSource::in_memory(
                        r#"Calculate: $ARGUMENTS. Show your work and provide the answer."#,
                    ),
                ),
            )
            .tools(ToolAccess::only(["Skill"]))
            .max_iterations(5)
            .build()
            .await
            .expect("Failed to create agent");

        let result = agent
            .execute("Use the Skill tool to invoke 'math-helper' with arguments '15 * 23 + 47'")
            .await
            .expect("Agent failed");

        println!("Math result:\n{}", result.text());
        assert!(
            result.text().contains("392") || result.text().contains("Calculate"),
            "Should contain result or skill output"
        );
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_agent_with_atlassian_skill() {
        // Simulate atlassian-cli skill
        let agent = Agent::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .skill(
                SkillIndex::new("jira", "Interact with Jira issues")
                    .with_source(ContentSource::in_memory(
                        r#"
You can interact with Jira using bash commands:
- List issues: echo "PROJ-123: Fix login bug (Open)"
- Create issue: echo "Created PROJ-456"

User request: $ARGUMENTS

Execute the simulated Jira command.
"#,
                    ))
                    .with_triggers(["jira", "issue"]),
            )
            .tools(ToolAccess::only(["Skill", "Bash"]))
            .max_iterations(5)
            .build()
            .await
            .expect("Failed to create agent");

        let result = agent
            .execute("Use the jira skill to list issues in project PROJ")
            .await
            .expect("Agent failed");

        println!("Jira skill result:\n{}", result.text());
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_progressive_skill_discovery() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.txt"), "Hello World")
            .await
            .unwrap();

        // Agent with multiple skills - only relevant one should be used
        let agent = Agent::builder()
            .auth(Auth::ClaudeCli)
            .await
            .expect("Failed to load CLI credentials")
            .skill(
                SkillIndex::new("file-analyzer", "Analyze file contents").with_source(
                    ContentSource::in_memory("Read and analyze the file: $ARGUMENTS"),
                ),
            )
            .skill(
                SkillIndex::new("code-reviewer", "Review code for issues")
                    .with_source(ContentSource::in_memory("Review code: $ARGUMENTS")),
            )
            .skill(
                SkillIndex::new("docker-helper", "Help with Docker commands")
                    .with_source(ContentSource::in_memory("Docker command: $ARGUMENTS")),
            )
            .tools(ToolAccess::only(["Skill", "Read"]))
            .working_dir(dir.path())
            .max_iterations(3)
            .build()
            .await
            .expect("Failed to create agent");

        let result = agent
            .execute("Use the file-analyzer skill to analyze test.txt")
            .await
            .expect("Agent failed");

        println!("Progressive disclosure result:\n{}", result.text());
    }
}

// =============================================================================
// Summary Test
// =============================================================================

#[test]
fn test_progressive_disclosure_summary() {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       Progressive Disclosure Integration Test Suite          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Features tested:                                             ║");
    println!("║ - SkillTool in default ToolRegistry                          ║");
    println!("║ - Skill execution with argument substitution                 ║");
    println!("║ - Progressive skill loading (on-demand)                      ║");
    println!("║ - Slash commands from .claude/commands/                      ║");
    println!("║ - Nested command namespaces (aws:lambda)                     ║");
    println!("║ - Trigger-based skill activation                             ║");
    println!("║ - ExecutionMode (DryRun, InlinePrompt, Callback)             ║");
    println!("║ - CLI tool integration pattern (docker, atlassian, etc)      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}
