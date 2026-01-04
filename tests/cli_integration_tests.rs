//! Comprehensive CLI Authentication Integration Tests
//!
//! These tests verify that the SDK works correctly with Claude Code CLI tokens,
//! including:
//! - Credential types and expiry handling
//! - All tool definitions and usage
//! - Progressive disclosure patterns
//! - OAuthConfig configuration

use claude_agent::client::messages::RequestMetadata;
use claude_agent::types::{Message, SystemBlock, SystemPrompt};
use claude_agent::{
    Client, Credential, OAuthConfig, ToolAccess, ToolRegistry, auth::OAuthCredential,
};

// =============================================================================
// Test 1: Credential Types
// =============================================================================

mod credential_tests {
    use super::*;

    #[test]
    fn test_api_key_credential() {
        let cred = Credential::api_key("sk-ant-api-test");
        assert!(!cred.is_expired());
        assert!(!cred.needs_refresh());
        assert_eq!(cred.credential_type(), "api_key");
    }

    #[test]
    fn test_oauth_credential() {
        let cred = Credential::oauth("sk-ant-oat01-test");
        assert_eq!(cred.credential_type(), "oauth");
    }

    #[test]
    fn test_credential_factory_methods() {
        let api_key = Credential::api_key("test-key");
        assert!(matches!(api_key, Credential::ApiKey(_)));

        let oauth = Credential::oauth("test-token");
        assert!(matches!(oauth, Credential::OAuth(_)));
    }
}

// =============================================================================
// Test 2: OAuthCredential Expiry Handling
// =============================================================================

mod credential_expiry_tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_expired_credential_detection() {
        let expired = OAuthCredential {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(0), // Unix epoch - definitely expired
            scopes: vec![],
            subscription_type: None,
        };

        assert!(expired.is_expired());
        assert!(expired.needs_refresh());
    }

    #[test]
    fn test_valid_credential() {
        let future_time = Utc::now().timestamp() + 7200; // 2 hours from now
        let valid = OAuthCredential {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(future_time),
            scopes: vec![],
            subscription_type: None,
        };

        assert!(!valid.is_expired());
        assert!(!valid.needs_refresh());
    }

    #[test]
    fn test_soon_to_expire_credential() {
        let soon = Utc::now().timestamp() + 60; // 1 minute from now
        let expiring = OAuthCredential {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(soon),
            scopes: vec![],
            subscription_type: None,
        };

        assert!(!expiring.is_expired());
        assert!(expiring.needs_refresh()); // Within 5-minute window
    }

    #[test]
    fn test_no_expiry_credential() {
        let no_expiry = OAuthCredential {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        };

        assert!(!no_expiry.is_expired());
        assert!(!no_expiry.needs_refresh());
    }

    #[test]
    fn test_credential_with_expiry_wrapping() {
        let cred = Credential::OAuth(OAuthCredential {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some(0),
            scopes: vec![],
            subscription_type: None,
        });

        assert!(cred.is_expired());
        assert!(cred.needs_refresh());
    }
}

// =============================================================================
// Test 3: Request Metadata Generation
// =============================================================================

mod metadata_tests {
    use super::*;

    #[test]
    fn test_metadata_generation() {
        let metadata = RequestMetadata::generate();

        assert!(metadata.user_id.is_some(), "user_id should be generated");

        let user_id = metadata.user_id.unwrap();
        assert!(user_id.starts_with("user_"), "Should start with user_");
        assert!(user_id.contains("_account_"), "Should contain account");
        assert!(user_id.contains("_session_"), "Should contain session");
    }

    #[test]
    fn test_metadata_uniqueness() {
        let m1 = RequestMetadata::generate();
        let m2 = RequestMetadata::generate();

        assert_ne!(
            m1.user_id, m2.user_id,
            "Each generation should produce unique IDs"
        );
    }
}

// =============================================================================
// Test 4: Tool Registry and Definitions
// =============================================================================

mod tool_registry_tests {
    use super::*;

    #[test]
    fn test_default_tools_registered() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);

        // Core file tools
        assert!(registry.contains("Read"), "Read tool should be registered");
        assert!(
            registry.contains("Write"),
            "Write tool should be registered"
        );
        assert!(registry.contains("Edit"), "Edit tool should be registered");
        assert!(registry.contains("Glob"), "Glob tool should be registered");
        assert!(registry.contains("Grep"), "Grep tool should be registered");

        // Shell tools
        assert!(registry.contains("Bash"), "Bash tool should be registered");

        // Productivity tools
        assert!(
            registry.contains("TodoWrite"),
            "TodoWrite tool should be registered"
        );

        // Web tools (WebSearch, WebFetch are now server-side tools via API)
        // They are not registered locally but injected into API requests

        // Plan mode tools
        assert!(registry.contains("Plan"), "Plan tool should be registered");
    }

    #[test]
    fn test_tool_access_filtering() {
        // Only allow Read and Write
        let registry =
            ToolRegistry::default_tools(&ToolAccess::only(["Read", "Write"]), None, None);

        assert!(registry.contains("Read"));
        assert!(registry.contains("Write"));
        assert!(!registry.contains("Bash"));
        assert!(!registry.contains("Grep"));
    }

    #[test]
    fn test_tool_access_except() {
        // Deny Bash only
        let registry = ToolRegistry::default_tools(&ToolAccess::except(["Bash"]), None, None);

        assert!(registry.contains("Read"));
        assert!(registry.contains("Write"));
        assert!(!registry.contains("Bash"));
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);

        for def in registry.definitions() {
            assert!(!def.name.is_empty(), "Tool name should not be empty");
            assert!(
                !def.description.is_empty(),
                "Tool description should not be empty: {}",
                def.name
            );
            assert!(
                def.input_schema.is_object(),
                "Input schema should be an object: {}",
                def.name
            );
        }
    }

    #[test]
    fn test_read_tool_schema() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);
        let read_tool = registry.get("Read").expect("Read tool should exist");
        let def = read_tool.definition();

        let schema = def.input_schema;
        let props = schema.get("properties").expect("Should have properties");
        assert!(
            props.get("file_path").is_some(),
            "Should have file_path property"
        );

        let required = schema.get("required").expect("Should have required");
        assert!(
            required
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("file_path")),
            "file_path should be required"
        );
    }

    #[test]
    fn test_edit_tool_schema() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);
        let edit_tool = registry.get("Edit").expect("Edit tool should exist");
        let def = edit_tool.definition();

        let schema = def.input_schema;
        let props = schema.get("properties").expect("Should have properties");

        assert!(props.get("file_path").is_some());
        assert!(props.get("old_string").is_some());
        assert!(props.get("new_string").is_some());
    }

    #[test]
    fn test_bash_tool_schema() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);
        let bash_tool = registry.get("Bash").expect("Bash tool should exist");
        let def = bash_tool.definition();

        let schema = def.input_schema;
        let props = schema.get("properties").expect("Should have properties");

        // Required properties
        assert!(
            props.get("command").is_some(),
            "Bash should have command property"
        );
        assert!(
            props.get("timeout").is_some(),
            "Bash should have timeout property"
        );

        // command should be required
        let required = schema.get("required").expect("Should have required");
        assert!(
            required
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("command")),
            "command should be required"
        );
    }
}

// =============================================================================
// Test 5: OAuthConfig and BetaConfig
// =============================================================================

mod oauth_config_tests {
    use super::*;
    use claude_agent::{BetaConfig, BetaFeature, ProviderConfig};

    #[test]
    fn test_oauth_default_values() {
        let config = OAuthConfig::default();

        assert!(config.user_agent.contains("claude-cli"));
        assert_eq!(config.app_identifier, "cli");
        assert!(config.url_params.contains_key("beta"));
    }

    #[test]
    fn test_oauth_builder() {
        let config = OAuthConfig::builder()
            .user_agent("test-agent/1.0")
            .app_identifier("test-app")
            .url_param("custom", "value")
            .header("X-Custom", "header")
            .build();

        assert_eq!(config.user_agent, "test-agent/1.0");
        assert_eq!(config.app_identifier, "test-app");
        assert!(config.url_params.contains_key("custom"));
        assert!(config.extra_headers.contains_key("X-Custom"));
    }

    #[test]
    fn test_beta_config_with_features() {
        let config = BetaConfig::new()
            .with(BetaFeature::InterleavedThinking)
            .with(BetaFeature::ContextManagement);

        let header = config.header_value().unwrap();
        assert!(header.contains("interleaved-thinking"));
        assert!(header.contains("context-management"));
    }

    #[test]
    fn test_beta_config_custom_flag() {
        let config = BetaConfig::new().with_custom("custom-flag-2025");

        let header = config.header_value().unwrap();
        assert!(header.contains("custom-flag-2025"));
    }

    #[test]
    fn test_provider_config_beta() {
        // with_beta_config replaces entire config, so include all features in it
        let config = ProviderConfig::default().with_beta_config(
            BetaConfig::new()
                .with(BetaFeature::InterleavedThinking)
                .with_custom("experimental-2026"),
        );

        assert!(config.beta.has(BetaFeature::InterleavedThinking));
        let header = config.beta.header_value().unwrap();
        assert!(header.contains("experimental-2026"));
    }
}

// =============================================================================
// Test 6: Tool Execution (Local Tests)
// =============================================================================

mod tool_execution_tests {
    use super::*;
    use claude_agent::PermissionPolicy;
    use tempfile::tempdir;
    use tokio::fs;

    fn permissive_policy() -> Option<PermissionPolicy> {
        Some(PermissionPolicy::permissive())
    }

    #[tokio::test]
    async fn test_read_tool_execution() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!\nLine 2")
            .await
            .unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            permissive_policy(),
        );

        let result = registry
            .execute(
                "Read",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap()
                }),
            )
            .await;

        match &result.output {
            claude_agent::tools::ToolOutput::Success(content) => {
                assert!(content.contains("Hello, World!"));
                assert!(content.contains("Line 2"));
            }
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_write_tool_execution() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("existing_file.txt");

        // Write tool overwrites existing files, so create one first
        fs::write(&file_path, "old content").await.unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            permissive_policy(),
        );

        let result = registry
            .execute(
                "Write",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "New content"
                }),
            )
            .await;

        assert!(!result.is_error(), "Write failed: {:?}", result);
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "New content");
    }

    #[tokio::test]
    async fn test_glob_tool_execution() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file1.txt"), "content")
            .await
            .unwrap();
        fs::write(dir.path().join("file2.txt"), "content")
            .await
            .unwrap();
        fs::write(dir.path().join("file3.rs"), "content")
            .await
            .unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            permissive_policy(),
        );

        let result = registry
            .execute(
                "Glob",
                serde_json::json!({
                    "pattern": "*.txt",
                    "path": dir.path().to_str().unwrap()
                }),
            )
            .await;

        match &result.output {
            claude_agent::tools::ToolOutput::Success(content) => {
                assert!(content.contains("file1.txt"));
                assert!(content.contains("file2.txt"));
                assert!(!content.contains("file3.rs"));
            }
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_grep_tool_execution() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("file.txt"),
            "Hello World\nfoo bar\nWorld Hello",
        )
        .await
        .unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            permissive_policy(),
        );

        let result = registry
            .execute(
                "Grep",
                serde_json::json!({
                    "pattern": "World",
                    "path": dir.path().to_str().unwrap()
                }),
            )
            .await;

        match &result.output {
            claude_agent::tools::ToolOutput::Success(content) => {
                assert!(content.contains("file.txt"));
            }
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_edit_tool_execution() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("edit_test.txt");
        fs::write(&file_path, "Hello OLD World").await.unwrap();

        let registry = ToolRegistry::default_tools(
            &ToolAccess::All,
            Some(dir.path().to_path_buf()),
            permissive_policy(),
        );

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

        assert!(!result.is_error());
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello NEW World");
    }

    #[tokio::test]
    async fn test_unknown_tool_error() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);

        let result = registry
            .execute("NonexistentTool", serde_json::json!({}))
            .await;

        assert!(result.is_error());
        match &result.output {
            claude_agent::tools::ToolOutput::Error(e) => {
                assert!(e.to_string().contains("Unknown tool"));
            }
            _ => panic!("Expected error"),
        }
    }
}

// =============================================================================
// Test 7: Client Builder Integration
// =============================================================================

mod client_builder_tests {
    use super::*;
    use claude_agent::Auth;

    #[tokio::test]
    async fn test_builder_with_api_key() {
        // This should not fail even without network
        let _builder = Client::builder()
            .auth("sk-ant-api-test-key")
            .await
            .expect("Auth failed");
        // Just verify builder works (actual build would need valid credentials)
    }

    #[tokio::test]
    async fn test_builder_with_oauth_token() {
        let _builder = Client::builder()
            .auth(Auth::oauth("sk-ant-oat01-test-token"))
            .await
            .expect("Auth failed");
    }

    #[tokio::test]
    async fn test_builder_with_credential() {
        let cred = Credential::api_key("test-key");
        let _builder = Client::builder().auth(cred).await.expect("Auth failed");
    }

    #[tokio::test]
    async fn test_builder_with_oauth_config() {
        let config = OAuthConfig::builder().user_agent("test-agent/1.0").build();

        let _builder = Client::builder()
            .auth(Auth::oauth("test-token"))
            .await
            .expect("Auth failed")
            .oauth_config(config);
    }
}

// =============================================================================
// Test 8: Progressive Disclosure - Tool Definitions
// =============================================================================

mod progressive_disclosure_tests {
    use super::*;

    #[test]
    fn test_tool_descriptions_are_concise() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);

        for def in registry.definitions() {
            // Tool descriptions should be useful but not overwhelming
            let desc_len = def.description.len();
            assert!(
                desc_len > 20,
                "Tool {} description too short: {} chars",
                def.name,
                desc_len
            );
            // Claude Code compatible: TodoWrite has ~9700 chars, Bash has ~5700 chars with git/PR examples
            // Other tools should still be reasonably sized
            let max_len = match def.name.as_str() {
                "TodoWrite" => 12000,
                "Bash" => 6000,
                "Plan" => 4000,
                _ => 4000,
            };
            assert!(
                desc_len < max_len,
                "Tool {} description may be too long: {} chars (max: {})",
                def.name,
                desc_len,
                max_len
            );
        }
    }

    #[test]
    fn test_input_schemas_are_complete() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);

        for def in registry.definitions() {
            let schema = &def.input_schema;

            // Every tool should have a type
            assert!(
                schema.get("type").is_some(),
                "Tool {} schema missing type",
                def.name
            );

            // Every tool should have properties (even if empty)
            assert!(
                schema.get("properties").is_some(),
                "Tool {} schema missing properties",
                def.name
            );
        }
    }

    #[test]
    fn test_tool_count_reasonable() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None, None);
        let tool_count = registry.names().len();

        // Should have reasonable number of tools for progressive disclosure
        assert!(tool_count >= 5, "Too few tools registered: {}", tool_count);
        assert!(
            tool_count <= 20,
            "Too many tools may hurt progressive disclosure: {}",
            tool_count
        );

        println!("Registered tools ({}): {:?}", tool_count, registry.names());
    }
}

// =============================================================================
// Test 9: System Prompt Types
// =============================================================================

mod system_prompt_tests {
    use super::*;

    #[test]
    fn test_system_prompt_text() {
        let prompt = SystemPrompt::text("You are a helpful assistant.");
        match prompt {
            SystemPrompt::Text(text) => {
                assert_eq!(text, "You are a helpful assistant.");
            }
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_system_prompt_blocks() {
        let blocks = vec![SystemBlock {
            block_type: "text".to_string(),
            text: "Block content".to_string(),
            cache_control: None,
        }];
        let prompt = SystemPrompt::Blocks(blocks);

        match prompt {
            SystemPrompt::Blocks(b) => {
                assert_eq!(b.len(), 1);
                assert_eq!(b[0].text, "Block content");
            }
            _ => panic!("Expected Blocks variant"),
        }
    }

    #[test]
    fn test_message_creation() {
        use claude_agent::types::Role;

        let user_msg = Message::user("Hello");
        assert!(matches!(user_msg.role, Role::User));

        let assistant_msg = Message::assistant("Hi there");
        assert!(matches!(assistant_msg.role, Role::Assistant));
    }
}
