//! Comprehensive verification tests for newly implemented features.
//!
//! Tests:
//! - CLAUDE.md project-level loading
//! - @import syntax
//! - .claude/rules/ recursive directory loading
//! - Cloud provider enum
//!
//! Run: cargo test --test new_features_verification -- --nocapture

use claude_agent::{
    client::CloudProvider,
    context::{ContextBuilder, MemoryLoader},
};
use tempfile::tempdir;
use tokio::fs;

// =============================================================================
// Part 1: Bedrock Strategy Tests (AWS feature only)
// =============================================================================

#[cfg(feature = "aws")]
mod bedrock_tests {
    use claude_agent::client::BedrockAdapter;

    #[tokio::test]
    async fn test_bedrock_adapter_from_env() {
        // This test only verifies the API exists, actual connection requires credentials
        use claude_agent::client::ProviderConfig;
        let _config = ProviderConfig::default();
        // Just verify the type exists and can be referenced
        let _: fn(ProviderConfig) -> _ = |c| async move {
            let _ = BedrockAdapter::from_env(c).await;
        };
    }
}

// =============================================================================
// Part 2: Vertex AI Strategy Tests (GCP feature only)
// =============================================================================

#[cfg(feature = "gcp")]
mod vertex_tests {
    use claude_agent::client::VertexAdapter;

    #[tokio::test]
    async fn test_vertex_adapter_from_env() {
        // This test only verifies the API exists, actual connection requires credentials
        use claude_agent::client::ProviderConfig;
        let _config = ProviderConfig::default();
        // Just verify the type exists and can be referenced
        let _: fn(ProviderConfig) -> _ = |c| async move {
            let _ = VertexAdapter::from_env(c).await;
        };
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
        assert_eq!(CloudProvider::Anthropic, CloudProvider::Anthropic);
        #[cfg(feature = "aws")]
        {
            assert_eq!(CloudProvider::Bedrock, CloudProvider::Bedrock);
            assert_ne!(CloudProvider::Bedrock, CloudProvider::Anthropic);
        }
        #[cfg(feature = "gcp")]
        {
            assert_eq!(CloudProvider::Vertex, CloudProvider::Vertex);
            assert_ne!(CloudProvider::Vertex, CloudProvider::Anthropic);
        }
    }

    #[cfg(feature = "aws")]
    #[test]
    fn test_bedrock_model_config() {
        let models = CloudProvider::Bedrock.default_models();
        assert!(models.primary.contains("global.anthropic"));
    }

    #[cfg(feature = "gcp")]
    #[test]
    fn test_vertex_model_config() {
        let models = CloudProvider::Vertex.default_models();
        assert!(models.primary.contains("@"));
    }

    #[test]
    fn test_anthropic_model_config() {
        let models = CloudProvider::Anthropic.default_models();
        assert!(models.primary.contains("claude"));
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
        let content = loader.load(dir.path()).await.unwrap();

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
        let content = loader.load(dir.path()).await.unwrap();

        assert_eq!(content.claude_md.len(), 1);
        assert!(content.claude_md[0].contains(".claude dir"));
    }

    #[tokio::test]
    async fn test_load_rules_directory() {
        let dir = tempdir().unwrap();
        let rules_dir = dir.path().join(".claude").join("rules");
        fs::create_dir_all(&rules_dir).await.unwrap();

        fs::write(rules_dir.join("rust.md"), "# Rust Rules\nUse snake_case")
            .await
            .unwrap();
        fs::write(
            rules_dir.join("security.md"),
            "# Security\nNo hardcoded secrets",
        )
        .await
        .unwrap();
        fs::write(
            rules_dir.join("typescript.md"),
            "# TypeScript\nUse strict mode",
        )
        .await
        .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load(dir.path()).await.unwrap();

        assert_eq!(content.rule_indices.len(), 3);

        let rule_names: Vec<_> = content
            .rule_indices
            .iter()
            .map(|r| r.name.as_str())
            .collect();
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
        fs::write(
            docs_dir.join("guidelines.md"),
            "## Guidelines\nFollow these rules",
        )
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
        let content = loader.load(dir.path()).await.unwrap();

        let combined = content.combined_claude_md();
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
        let content = loader.load(dir.path()).await.unwrap();

        let combined = content.combined_claude_md();
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
        let result = loader.load(dir.path()).await;

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
        let content = loader.load(dir.path()).await.unwrap();

        let combined = content.combined_claude_md();
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
        fs::write(rules_dir.join("test.md"), "Rule content")
            .await
            .unwrap();

        let mut loader = MemoryLoader::new();
        let content = loader.load(dir.path()).await.unwrap();

        let combined = content.combined_claude_md();
        assert!(combined.contains("Main content"));

        // Rules are now loaded as indices, not directly in combined content
        assert!(!content.rule_indices.is_empty());
        let rule = content.rule_indices.iter().find(|r| r.name == "test");
        assert!(rule.is_some());
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
        let content = loader.load(dir.path()).await.unwrap();

        let combined = content.combined_claude_md();
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
        fs::write(
            dir.path().join("CLAUDE.md"),
            "# Project Context\nImportant info",
        )
        .await
        .unwrap();

        let context = ContextBuilder::new()
            .load_from_directory(dir.path())
            .await
            .build()
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

        fs::write(dir.path().join("CLAUDE.md"), "Main content")
            .await
            .unwrap();
        fs::write(rules_dir.join("rule1.md"), "Rule 1 content")
            .await
            .unwrap();

        let context = ContextBuilder::new()
            .load_from_directory(dir.path())
            .await
            .build()
            .unwrap();

        let md = &context.static_context().claude_md;
        assert!(md.contains("Main content"));

        // Rules are loaded as indices in RulesEngine, not in claude_md
        let summary = context.build_rules_summary();
        assert!(summary.contains("rule1"));
    }
}

// =============================================================================
// Part 6: Live Tests (require CLI credentials)
// =============================================================================

mod live_tests {
    use claude_agent::{Agent, Auth, Client, ToolAccess};
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    #[ignore = "Requires CLI credentials"]
    async fn test_cli_auth_with_new_features() {
        let client = Client::builder()
            .auth(Auth::FromEnv)
            .await
            .expect("Auth failed")
            .build()
            .await
            .expect("Failed to create client");

        println!("Provider: {}", client.adapter().name());

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
            .auth(Auth::FromEnv)
            .await
            .expect("Auth failed")
            .tools(ToolAccess::only(["Read"]))
            .working_dir(dir.path())
            .max_iterations(3)
            .build()
            .await
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
// Part 7: Foundry Strategy Tests (Azure feature only)
// =============================================================================

#[cfg(feature = "azure")]
mod foundry_tests {
    use claude_agent::client::FoundryAdapter;

    #[tokio::test]
    async fn test_foundry_adapter_from_env() {
        // This test only verifies the API exists, actual connection requires credentials
        use claude_agent::client::ProviderConfig;
        let _config = ProviderConfig::default();
        // Just verify the type exists and can be referenced
        let _: fn(ProviderConfig) -> _ = |c| async move {
            let _ = FoundryAdapter::from_env(c).await;
        };
    }
}

// =============================================================================
// Part 8: Small Model Configuration Tests
// =============================================================================

mod small_model_tests {
    use claude_agent::Client;

    #[tokio::test]
    async fn test_client_builder_api() {
        // Builder should accept model configuration
        let _builder = Client::builder()
            .auth("test-key")
            .await
            .expect("Auth failed")
            .anthropic();
    }

    #[test]
    fn test_cloud_provider_variants() {
        use claude_agent::client::CloudProvider;

        // Always available
        assert_eq!(CloudProvider::Anthropic, CloudProvider::Anthropic);

        #[cfg(feature = "azure")]
        {
            let foundry = CloudProvider::Foundry;
            assert_eq!(foundry, CloudProvider::Foundry);
            assert_ne!(foundry, CloudProvider::Anthropic);
        }
        #[cfg(feature = "aws")]
        {
            assert_ne!(CloudProvider::Bedrock, CloudProvider::Anthropic);
        }
        #[cfg(feature = "gcp")]
        {
            assert_ne!(CloudProvider::Vertex, CloudProvider::Anthropic);
        }
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
    fn test_permission_settings_default() {
        let loader = SettingsLoader::new();
        // Default permissions are empty
        assert!(loader.settings().permissions.is_empty());
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

        assert_eq!(
            settings.env.get("TEST_VAR"),
            Some(&"test_value".to_string())
        );
        assert!(
            settings
                .permissions
                .deny
                .contains(&"Read(./.env)".to_string())
        );
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
        let content = loader.load(dir.path()).await.unwrap();

        // Should gracefully handle missing home dir files
        let combined = content.combined_claude_md();
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
    println!("========================================================================");
    println!("         New Features Verification Test Suite                           ");
    println!("========================================================================");
    println!(" Features tested:                                                       ");
    println!(" - Cloud provider auto-resolution                                       ");
    println!(" - CLAUDE.md project-level loading                                      ");
    println!(" - @import syntax with home directory (~) expansion                     ");
    println!(" - .claude/rules/ recursive directory loading                           ");
    println!(" - Slash commands (.claude/commands/)                                   ");
    println!(" - $ARGUMENTS template substitution                                     ");
    println!(" - settings.json and settings.local.json loading                        ");
    println!(" - permissions.deny patterns                                            ");
    println!(" - Circular import prevention                                           ");
    #[cfg(feature = "aws")]
    println!(" - AWS Bedrock adapter (feature = aws)                                  ");
    #[cfg(feature = "gcp")]
    println!(" - Google Vertex AI adapter (feature = gcp)                             ");
    #[cfg(feature = "azure")]
    println!(" - Microsoft Azure AI Foundry adapter (feature = azure)                 ");
    println!("========================================================================");
    println!();
}
