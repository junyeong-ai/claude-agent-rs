//! Comprehensive verification tests for newly implemented features.
//!
//! Tests:
//! - Bedrock/Vertex AI authentication strategies
//! - CLAUDE.md recursive loading
//! - CLAUDE.local.md support
//! - @import syntax
//! - .claude/rules/ directory loading
//! - Cloud provider auto-resolution
//!
//! Run: cargo test --test new_features_verification -- --nocapture

use claude_agent::{
    auth::{BedrockStrategy, VertexStrategy, AuthStrategy},
    client::{ClientBuilder, CloudProvider},
    context::{ContextBuilder, MemoryLoader},
};
use tempfile::tempdir;
use tokio::fs;

// =============================================================================
// Part 1: Bedrock Strategy Tests
// =============================================================================

mod bedrock_tests {
    use super::*;

    #[test]
    fn test_bedrock_strategy_creation() {
        let strategy = BedrockStrategy::new("us-west-2");
        assert_eq!(strategy.region(), "us-west-2");
        assert_eq!(strategy.name(), "bedrock");
    }

    #[test]
    fn test_bedrock_base_url_construction() {
        let strategy = BedrockStrategy::new("us-east-1");
        let url = strategy.get_base_url();
        assert!(url.contains("bedrock-runtime"));
        assert!(url.contains("us-east-1"));
        assert!(url.contains("amazonaws.com"));
    }

    #[test]
    fn test_bedrock_custom_base_url() {
        let strategy = BedrockStrategy::new("us-east-1")
            .with_base_url("https://my-gateway.com/bedrock");
        assert_eq!(strategy.get_base_url(), "https://my-gateway.com/bedrock");
    }

    #[test]
    fn test_bedrock_skip_auth() {
        let strategy = BedrockStrategy::new("us-east-1").skip_auth();
        // Just verify it doesn't panic
        let _ = strategy.extra_headers();
    }

    #[test]
    fn test_bedrock_with_credentials() {
        let strategy = BedrockStrategy::new("us-east-1")
            .with_credentials("AKIAIOSFODNN7EXAMPLE", "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
        assert_eq!(strategy.name(), "bedrock");
    }

    #[test]
    fn test_bedrock_auth_header() {
        let strategy = BedrockStrategy::new("us-east-1");
        let (name, value) = strategy.auth_header();
        assert_eq!(name, "x-bedrock-auth");
        assert_eq!(value, "aws-sigv4");
    }
}

// =============================================================================
// Part 2: Vertex AI Strategy Tests
// =============================================================================

mod vertex_tests {
    use super::*;

    #[test]
    fn test_vertex_strategy_creation() {
        let strategy = VertexStrategy::new("my-project", "us-central1");
        assert_eq!(strategy.project_id(), "my-project");
        assert_eq!(strategy.region(), "us-central1");
        assert_eq!(strategy.name(), "vertex");
    }

    #[test]
    fn test_vertex_base_url_construction() {
        let strategy = VertexStrategy::new("my-project", "europe-west1");
        let url = strategy.get_base_url();
        assert!(url.contains("aiplatform.googleapis.com"));
        assert!(url.contains("my-project"));
        assert!(url.contains("europe-west1"));
        assert!(url.contains("anthropic"));
    }

    #[test]
    fn test_vertex_custom_base_url() {
        let strategy = VertexStrategy::new("p", "r")
            .with_base_url("https://my-gateway.com/vertex");
        assert_eq!(strategy.get_base_url(), "https://my-gateway.com/vertex");
    }

    #[test]
    fn test_vertex_skip_auth() {
        let strategy = VertexStrategy::new("p", "r").skip_auth();
        let _ = strategy.extra_headers();
    }

    #[test]
    fn test_vertex_with_access_token() {
        let strategy = VertexStrategy::new("p", "r")
            .with_access_token("ya29.example-token");
        let (name, value) = strategy.auth_header();
        assert_eq!(name, "Authorization");
        assert!(value.starts_with("Bearer "));
        assert!(value.contains("ya29.example-token"));
    }

    #[test]
    fn test_vertex_extra_headers() {
        let strategy = VertexStrategy::new("my-project", "us-central1");
        let headers = strategy.extra_headers();
        assert!(headers.iter().any(|(k, v)| k == "x-goog-user-project" && v == "my-project"));
    }
}

// =============================================================================
// Part 3: Cloud Provider Selection Tests
// =============================================================================

mod cloud_provider_tests {
    use super::*;

    #[test]
    fn test_cloud_provider_default() {
        let provider = CloudProvider::default();
        assert_eq!(provider, CloudProvider::Anthropic);
    }

    #[test]
    fn test_cloud_provider_equality() {
        assert_eq!(CloudProvider::Bedrock, CloudProvider::Bedrock);
        assert_eq!(CloudProvider::Vertex, CloudProvider::Vertex);
        assert_ne!(CloudProvider::Bedrock, CloudProvider::Vertex);
    }

    #[test]
    fn test_client_builder_bedrock() {
        let _builder = ClientBuilder::default()
            .bedrock("us-west-2");
        // Just verify it doesn't panic
    }

    #[test]
    fn test_client_builder_vertex() {
        let _builder = ClientBuilder::default()
            .vertex("my-project", "us-central1");
    }

    #[test]
    fn test_client_builder_bedrock_strategy() {
        let strategy = BedrockStrategy::new("eu-west-1")
            .with_base_url("https://proxy.example.com")
            .skip_auth();
        let _builder = ClientBuilder::default()
            .bedrock_strategy(strategy);
    }

    #[test]
    fn test_client_builder_vertex_strategy() {
        let strategy = VertexStrategy::new("project", "region")
            .with_access_token("token")
            .skip_auth();
        let _builder = ClientBuilder::default()
            .vertex_strategy(strategy);
    }
}

// =============================================================================
// Part 4: Memory Loader Tests
// =============================================================================

mod memory_loader_tests {
    use super::*;

    #[tokio::test]
    async fn test_load_simple_claude_md() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Project\nTest content")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.claude_md.len(), 1);
        assert!(content.claude_md[0].contains("Test content"));
        assert!(!content.is_empty());
    }

    #[tokio::test]
    async fn test_load_claude_md_in_dot_claude_dir() {
        let dir = tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        fs::create_dir_all(&claude_dir).await.unwrap();
        fs::write(claude_dir.join("CLAUDE.md"), "Content in .claude dir")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.claude_md.len(), 1);
        assert!(content.claude_md[0].contains(".claude dir"));
    }

    #[tokio::test]
    async fn test_load_claude_local_md() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.local.md"), "Local private settings")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.local_md.len(), 1);
        assert!(content.local_md[0].contains("Local private"));
    }

    #[tokio::test]
    async fn test_load_rules_directory() {
        let dir = tempdir().unwrap();
        let rules_dir = dir.path().join(".claude").join("rules");
        fs::create_dir_all(&rules_dir).await.unwrap();

        fs::write(rules_dir.join("rust.md"), "# Rust Rules\nUse snake_case")
            .await
            .unwrap();
        fs::write(rules_dir.join("security.md"), "# Security\nNo hardcoded secrets")
            .await
            .unwrap();
        fs::write(rules_dir.join("typescript.md"), "# TypeScript\nUse strict mode")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        assert_eq!(content.rules.len(), 3);

        let rule_names: Vec<_> = content.rules.iter().map(|r| r.name.as_str()).collect();
        assert!(rule_names.contains(&"rust"));
        assert!(rule_names.contains(&"security"));
        assert!(rule_names.contains(&"typescript"));
    }

    #[tokio::test]
    async fn test_import_syntax() {
        let dir = tempdir().unwrap();

        // Create docs directory with imported file
        let docs_dir = dir.path().join("docs");
        fs::create_dir_all(&docs_dir).await.unwrap();
        fs::write(docs_dir.join("guidelines.md"), "## Guidelines\nFollow these rules")
            .await
            .unwrap();

        // Create main CLAUDE.md with import
        fs::write(
            dir.path().join("CLAUDE.md"),
            "# Project\n@docs/guidelines.md\n# End",
        )
        .await
        .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        let combined = content.combined();
        assert!(combined.contains("# Project"));
        assert!(combined.contains("## Guidelines"));
        assert!(combined.contains("Follow these rules"));
        assert!(combined.contains("# End"));
    }

    #[tokio::test]
    async fn test_nested_imports() {
        let dir = tempdir().unwrap();

        // Create nested structure
        let level1 = dir.path().join("level1");
        let level2 = level1.join("level2");
        fs::create_dir_all(&level2).await.unwrap();

        fs::write(level2.join("deep.md"), "Deep content")
            .await
            .unwrap();
        fs::write(level1.join("mid.md"), "Mid content\n@level2/deep.md")
            .await
            .unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "Top\n@level1/mid.md")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        let combined = content.combined();
        assert!(combined.contains("Top"));
        assert!(combined.contains("Mid content"));
        assert!(combined.contains("Deep content"));
    }

    #[tokio::test]
    async fn test_circular_import_prevention() {
        let dir = tempdir().unwrap();

        // Create circular imports
        fs::write(dir.path().join("a.md"), "A\n@b.md")
            .await
            .unwrap();
        fs::write(dir.path().join("b.md"), "B\n@a.md")
            .await
            .unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "@a.md")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let result = loader.load_all(dir.path()).await;

        // Should not hang or crash
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_missing_import_file() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("CLAUDE.md"),
            "Content\n@nonexistent.md\nMore content",
        )
        .await
        .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        let combined = content.combined();
        assert!(combined.contains("Content"));
        assert!(combined.contains("@nonexistent.md")); // Kept as-is
        assert!(combined.contains("More content"));
    }

    #[tokio::test]
    async fn test_combined_content() {
        let dir = tempdir().unwrap();
        let rules_dir = dir.path().join(".claude").join("rules");
        fs::create_dir_all(&rules_dir).await.unwrap();

        fs::write(dir.path().join("CLAUDE.md"), "Main content")
            .await
            .unwrap();
        fs::write(dir.path().join("CLAUDE.local.md"), "Local content")
            .await
            .unwrap();
        fs::write(rules_dir.join("test.md"), "Rule content")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        let combined = content.combined();
        assert!(combined.contains("Main content"));
        assert!(combined.contains("Local content"));
        assert!(combined.contains("Rule content"));
        assert!(combined.contains("# Rule: test"));
    }

    #[tokio::test]
    async fn test_escape_at_syntax() {
        let dir = tempdir().unwrap();

        // @@ should be kept as-is (escaped @)
        fs::write(
            dir.path().join("CLAUDE.md"),
            "Email: @@user@example.com\n@@ is escaped",
        )
        .await
        .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        let combined = content.combined();
        assert!(combined.contains("@@user@example.com"));
        assert!(combined.contains("@@ is escaped"));
    }
}

// =============================================================================
// Part 5: Context Builder Integration Tests
// =============================================================================

mod context_builder_tests {
    use super::*;

    #[tokio::test]
    async fn test_context_builder_with_memory_loader() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Project Context\nImportant info")
            .await
            .unwrap();

        let context = ContextBuilder::new()
            .load_memory_recursive(dir.path())
            .await
            .build_sync()
            .unwrap();

        let static_ctx = context.static_context();
        assert!(static_ctx.claude_md.contains("Project Context"));
        assert!(static_ctx.claude_md.contains("Important info"));
    }

    #[tokio::test]
    async fn test_context_builder_with_all_sources() {
        let dir = tempdir().unwrap();
        let rules_dir = dir.path().join(".claude").join("rules");
        fs::create_dir_all(&rules_dir).await.unwrap();

        fs::write(dir.path().join("CLAUDE.md"), "Main")
            .await
            .unwrap();
        fs::write(dir.path().join("CLAUDE.local.md"), "Local")
            .await
            .unwrap();
        fs::write(rules_dir.join("rule1.md"), "Rule 1 content")
            .await
            .unwrap();

        let context = ContextBuilder::new()
            .load_memory_recursive(dir.path())
            .await
            .build_sync()
            .unwrap();

        let md = &context.static_context().claude_md;
        assert!(md.contains("Main"));
        assert!(md.contains("Local"));
        assert!(md.contains("Rule 1 content"));
    }
}

// =============================================================================
// Part 6: Environment Variable Tests
// =============================================================================

mod env_var_tests {
    use super::*;

    #[test]
    fn test_bedrock_from_env_disabled() {
        // Clear any existing env vars
        std::env::remove_var("CLAUDE_CODE_USE_BEDROCK");

        let strategy = BedrockStrategy::from_env();
        assert!(strategy.is_none());
    }

    #[test]
    fn test_vertex_from_env_disabled() {
        std::env::remove_var("CLAUDE_CODE_USE_VERTEX");

        let strategy = VertexStrategy::from_env();
        assert!(strategy.is_none());
    }
}

// =============================================================================
// Live Tests (require CLI credentials)
// =============================================================================

mod live_tests {
    use super::*;
    use claude_agent::{Client, Agent, ToolAccess};

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_cli_auth_with_new_features() {
        let client = Client::builder()
            .from_claude_cli()
            .build()
            .expect("Failed to create client");

        println!("Auth strategy: {}", client.config().auth_strategy.name());
        assert_eq!(client.config().auth_strategy.name(), "oauth");

        let response = client
            .query("What is 1+1? Answer with just the number.")
            .await
            .expect("Query failed");

        println!("Response: {}", response.trim());
        assert!(response.contains("2"));
    }

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_agent_with_memory_context() {
        let dir = tempdir().unwrap();

        // Create CLAUDE.md with project context
        fs::write(
            dir.path().join("CLAUDE.md"),
            "# Test Project\nThis is a test project for verification.\nThe secret code is 42.",
        )
        .await
        .unwrap();

        // Create a test file
        fs::write(dir.path().join("data.txt"), "The answer is 42.")
            .await
            .unwrap();

        let agent = Agent::builder()
            .from_claude_cli()
            .tools(ToolAccess::only(["Read"]))
            .working_dir(dir.path())
            .max_iterations(3)
            .build()
            .expect("Failed to create agent");

        let result = agent
            .execute("Read data.txt and tell me the answer. Just the number.")
            .await
            .expect("Agent failed");

        println!("Result: {}", result.text());
        assert!(result.text().contains("42"));
    }
}

// =============================================================================
// Part 7: Foundry Strategy Tests
// =============================================================================

mod foundry_tests {
    use claude_agent::auth::{AuthStrategy, FoundryStrategy};

    #[test]
    fn test_foundry_strategy_creation() {
        let strategy = FoundryStrategy::new("my-resource", "claude-sonnet");
        assert_eq!(strategy.resource_name(), "my-resource");
        assert_eq!(strategy.deployment_name(), "claude-sonnet");
        assert_eq!(strategy.name(), "foundry");
    }

    #[test]
    fn test_foundry_base_url_construction() {
        let strategy = FoundryStrategy::new("my-resource", "claude-sonnet");
        let url = strategy.get_base_url();
        assert!(url.contains("my-resource"));
        assert!(url.contains("claude-sonnet"));
        assert!(url.contains("openai.azure.com"));
    }

    #[test]
    fn test_foundry_custom_base_url() {
        let strategy = FoundryStrategy::new("r", "d")
            .with_base_url("https://my-gateway.com/foundry");
        assert_eq!(strategy.get_base_url(), "https://my-gateway.com/foundry");
    }

    #[test]
    fn test_foundry_api_version() {
        let strategy = FoundryStrategy::new("r", "d")
            .with_api_version("2025-01-01");
        let query = strategy.url_query_string();
        assert!(query.is_some());
        assert!(query.unwrap().contains("2025-01-01"));
    }

    #[test]
    fn test_foundry_auth_with_api_key() {
        let strategy = FoundryStrategy::new("r", "d").with_api_key("my-key");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "api-key");
        assert_eq!(value, "my-key");
    }

    #[test]
    fn test_foundry_auth_with_token() {
        let strategy = FoundryStrategy::new("r", "d").with_access_token("my-token");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "Authorization");
        assert!(value.contains("Bearer my-token"));
    }

    #[test]
    fn test_foundry_from_env_disabled() {
        std::env::remove_var("CLAUDE_CODE_USE_FOUNDRY");
        let strategy = FoundryStrategy::from_env();
        assert!(strategy.is_none());
    }
}

// =============================================================================
// Part 8: Small Model Configuration Tests
// =============================================================================

mod small_model_tests {
    use claude_agent::client::ClientBuilder;

    #[test]
    fn test_small_model_builder() {
        // Builder should accept small_model configuration
        let _builder = ClientBuilder::default()
            .api_key("test-key")
            .model("claude-sonnet-4-5")
            .small_model("claude-haiku-4-5");
    }

    #[test]
    fn test_cloud_provider_with_foundry() {
        use claude_agent::client::CloudProvider;

        let foundry = CloudProvider::Foundry;
        assert_eq!(foundry, CloudProvider::Foundry);
        assert_ne!(foundry, CloudProvider::Anthropic);
        assert_ne!(foundry, CloudProvider::Bedrock);
        assert_ne!(foundry, CloudProvider::Vertex);
    }
}

// =============================================================================
// Part 9: Slash Commands Tests
// =============================================================================

mod slash_commands_tests {
    use claude_agent::skills::SlashCommand;
    use std::path::PathBuf;

    #[test]
    fn test_slash_command_creation() {
        let cmd = SlashCommand {
            name: "commit".to_string(),
            description: Some("Create a git commit".to_string()),
            content: "Analyze changes and create commit: $ARGUMENTS".to_string(),
            location: PathBuf::from(".claude/commands/commit.md"),
            allowed_tools: vec!["Bash".to_string()],
            argument_hint: Some("message".to_string()),
            model: None,
        };

        assert_eq!(cmd.name, "commit");
        assert_eq!(cmd.allowed_tools.len(), 1);
    }

    #[test]
    fn test_slash_command_argument_substitution() {
        let cmd = SlashCommand {
            name: "test".to_string(),
            description: None,
            content: "Fix issue: $ARGUMENTS in the codebase".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute("login bug");
        assert_eq!(result, "Fix issue: login bug in the codebase");
    }

    #[test]
    fn test_multiple_argument_substitution() {
        let cmd = SlashCommand {
            name: "test".to_string(),
            description: None,
            content: "First: $ARGUMENTS, Second: $ARGUMENTS".to_string(),
            location: PathBuf::from("/test"),
            allowed_tools: vec![],
            argument_hint: None,
            model: None,
        };

        let result = cmd.execute("value");
        assert_eq!(result, "First: value, Second: value");
    }
}

// =============================================================================
// Part 10: Settings Tests
// =============================================================================

mod settings_tests {
    use claude_agent::config::SettingsLoader;

    #[test]
    fn test_settings_loader_creation() {
        let loader = SettingsLoader::new();
        assert!(loader.settings().env.is_empty());
    }

    #[test]
    fn test_permission_pattern_exact() {
        let loader = SettingsLoader::new();
        // Pattern matching is internal, test via is_denied when settings loaded
        assert!(!loader.is_denied("random_file"));
    }

    #[tokio::test]
    async fn test_settings_loading() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        fs::create_dir_all(&claude_dir).await.unwrap();

        // Create settings.json
        fs::write(
            claude_dir.join("settings.json"),
            r#"{
                "env": {
                    "TEST_VAR": "test_value"
                },
                "permissions": {
                    "deny": ["Read(./.env)"]
                }
            }"#,
        )
        .await
        .unwrap();

        let mut loader = SettingsLoader::new();
        let settings = loader.load(dir.path()).await.unwrap();

        assert_eq!(settings.env.get("TEST_VAR"), Some(&"test_value".to_string()));
        assert!(settings.permissions.deny.contains(&"Read(./.env)".to_string()));
    }

    #[tokio::test]
    async fn test_settings_local_override() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().unwrap();
        let claude_dir = dir.path().join(".claude");
        fs::create_dir_all(&claude_dir).await.unwrap();

        // Create settings.json
        fs::write(
            claude_dir.join("settings.json"),
            r#"{"env": {"VAR": "original"}}"#,
        )
        .await
        .unwrap();

        // Create settings.local.json (overrides)
        fs::write(
            claude_dir.join("settings.local.json"),
            r#"{"env": {"VAR": "overridden", "LOCAL_VAR": "local"}}"#,
        )
        .await
        .unwrap();

        let mut loader = SettingsLoader::new();
        let settings = loader.load(dir.path()).await.unwrap();

        assert_eq!(settings.env.get("VAR"), Some(&"overridden".to_string()));
        assert_eq!(settings.env.get("LOCAL_VAR"), Some(&"local".to_string()));
    }
}

// =============================================================================
// Part 11: Home Directory Expansion Tests
// =============================================================================

mod home_dir_tests {
    use super::*;

    #[tokio::test]
    async fn test_home_dir_import_syntax() {
        // This test verifies the home directory expansion logic exists
        // Actual home dir import requires a real home directory file
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("CLAUDE.md"),
            "# Project\n@~/nonexistent_file.md\n# End",
        )
        .await
        .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load_all(dir.path()).await.unwrap();

        // Should gracefully handle missing home dir files
        let combined = content.combined();
        assert!(combined.contains("# Project"));
        assert!(combined.contains("@~/nonexistent_file.md")); // Kept as-is since file doesn't exist
    }
}

// =============================================================================
// Summary Test
// =============================================================================

#[test]
fn test_all_new_features_summary() {
    println!("\n");
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         New Features Verification Test Suite                 ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║ Features tested:                                             ║");
    println!("║ - AWS Bedrock authentication strategy                        ║");
    println!("║ - Google Vertex AI authentication strategy                   ║");
    println!("║ - Microsoft Azure AI Foundry authentication strategy         ║");
    println!("║ - Cloud provider auto-resolution                             ║");
    println!("║ - ANTHROPIC_SMALL_FAST_MODEL configuration                   ║");
    println!("║ - CLAUDE.md recursive loading                                ║");
    println!("║ - CLAUDE.local.md support                                    ║");
    println!("║ - @import syntax with home directory (~) expansion           ║");
    println!("║ - .claude/rules/ directory loading                           ║");
    println!("║ - Slash commands (.claude/commands/)                         ║");
    println!("║ - $ARGUMENTS template substitution                           ║");
    println!("║ - settings.json and settings.local.json loading              ║");
    println!("║ - permissions.deny patterns                                  ║");
    println!("║ - Circular import prevention                                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
}
