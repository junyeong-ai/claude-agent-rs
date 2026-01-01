//! Comprehensive CLI Authentication Integration Tests
//!
//! These tests verify that the SDK works correctly with Claude Code CLI tokens,
//! including:
//! - Authentication flow with OAuth tokens
//! - Prompt caching with ephemeral cache_control
//! - All tool definitions and usage
//! - Progressive disclosure patterns
//! - Streaming and non-streaming modes

use claude_agent::{
    auth::{ApiKeyStrategy, AuthStrategy, OAuthConfig, OAuthCredential, OAuthStrategy},
    client::messages::{CreateMessageRequest, RequestMetadata},
    types::{Message, SystemPrompt},
    Client, ToolAccess, ToolRegistry,
};
use std::collections::HashMap;

// =============================================================================
// Test 1: OAuth Authentication Strategy
// =============================================================================

mod oauth_strategy_tests {
    use super::*;

    fn create_test_oauth_credential() -> OAuthCredential {
        OAuthCredential {
            access_token: "sk-ant-oat01-test-token".to_string(),
            refresh_token: Some("refresh-token".to_string()),
            expires_at: Some(chrono::Utc::now().timestamp() + 3600),
            scopes: vec!["api".to_string()],
            subscription_type: Some("pro".to_string()),
        }
    }

    #[test]
    fn test_oauth_auth_header_format() {
        let cred = create_test_oauth_credential();
        let strategy = OAuthStrategy::new(cred);

        let (name, value) = strategy.auth_header();
        assert_eq!(name, "Authorization");
        assert!(value.starts_with("Bearer "));
        assert!(value.contains("sk-ant-oat01-test-token"));
    }

    #[test]
    fn test_oauth_extra_headers_complete() {
        let cred = create_test_oauth_credential();
        let strategy = OAuthStrategy::new(cred);

        let headers = strategy.extra_headers();
        let header_map: HashMap<_, _> = headers.into_iter().collect();

        // Required headers for Claude Code CLI
        assert!(
            header_map.contains_key("anthropic-beta"),
            "Missing anthropic-beta header"
        );
        assert!(
            header_map.contains_key("user-agent"),
            "Missing user-agent header"
        );
        assert!(header_map.contains_key("x-app"), "Missing x-app header");
        assert!(
            header_map.contains_key("anthropic-dangerous-direct-browser-access"),
            "Missing browser access header"
        );

        // Verify beta flags content
        let beta_value = header_map.get("anthropic-beta").unwrap();
        assert!(
            beta_value.contains("claude-code-20250219"),
            "Missing claude-code beta flag"
        );
        assert!(
            beta_value.contains("oauth-2025-04-20"),
            "Missing oauth beta flag"
        );
        assert!(
            beta_value.contains("interleaved-thinking-2025-05-14"),
            "Missing thinking beta flag"
        );
    }

    #[test]
    fn test_oauth_url_query_params() {
        let cred = create_test_oauth_credential();
        let strategy = OAuthStrategy::new(cred);

        let query = strategy.url_query_string();
        assert!(query.is_some(), "URL query params should be present");
        assert!(
            query.unwrap().contains("beta=true"),
            "Should include beta=true param"
        );
    }

    #[test]
    fn test_oauth_custom_config() {
        let cred = create_test_oauth_credential();
        let config = OAuthConfig::builder()
            .system_prompt("Custom Agent Prompt")
            .user_agent("my-agent/2.0")
            .add_beta_flag("custom-flag-2025")
            .build();

        let strategy = OAuthStrategy::with_config(cred, config);

        assert_eq!(strategy.config().system_prompt, "Custom Agent Prompt");
        assert_eq!(strategy.config().user_agent, "my-agent/2.0");
        assert!(strategy
            .config()
            .beta_flags
            .contains(&"custom-flag-2025".to_string()));
    }
}

// =============================================================================
// Test 2: Prompt Caching with Ephemeral Cache Control
// =============================================================================

mod prompt_caching_tests {
    use super::*;

    fn create_test_oauth_credential() -> OAuthCredential {
        OAuthCredential {
            access_token: "test-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        }
    }

    #[test]
    fn test_system_prompt_prepend_with_cache_control() {
        let cred = create_test_oauth_credential();
        let strategy = OAuthStrategy::new(cred);

        // Test with no existing prompt
        let result = strategy.prepare_system_prompt(None);
        assert!(result.is_some());

        if let Some(SystemPrompt::Blocks(blocks)) = result {
            assert_eq!(blocks.len(), 1);
            assert!(blocks[0].text.contains("Claude Code"));
            assert!(
                blocks[0].cache_control.is_some(),
                "Cache control should be set"
            );
            assert_eq!(
                blocks[0].cache_control.as_ref().unwrap().cache_type,
                "ephemeral"
            );
        } else {
            panic!("Expected Blocks variant");
        }
    }

    #[test]
    fn test_system_prompt_prepend_preserves_user_prompt() {
        let cred = create_test_oauth_credential();
        let strategy = OAuthStrategy::new(cred);

        // Test with existing text prompt
        let existing = SystemPrompt::text("User's custom instructions");
        let result = strategy.prepare_system_prompt(Some(existing));

        if let Some(SystemPrompt::Blocks(blocks)) = result {
            assert_eq!(blocks.len(), 2, "Should have Claude Code + user prompt");

            // First block: Claude Code prompt with cache
            assert!(blocks[0].text.contains("Claude Code"));
            assert!(blocks[0].cache_control.is_some());

            // Second block: User prompt without cache
            assert_eq!(blocks[1].text, "User's custom instructions");
            assert!(blocks[1].cache_control.is_none());
        } else {
            panic!("Expected Blocks variant");
        }
    }

    #[test]
    fn test_system_prompt_prepend_to_blocks() {
        let cred = create_test_oauth_credential();
        let strategy = OAuthStrategy::new(cred);

        // Test with existing blocks
        let existing = SystemPrompt::Blocks(vec![claude_agent::types::SystemBlock {
            block_type: "text".to_string(),
            text: "Existing block 1".to_string(),
            cache_control: None,
        }]);

        let result = strategy.prepare_system_prompt(Some(existing));

        if let Some(SystemPrompt::Blocks(blocks)) = result {
            assert_eq!(blocks.len(), 2, "Should prepend to existing blocks");
            assert!(
                blocks[0].text.contains("Claude Code"),
                "Claude Code should be first"
            );
            assert_eq!(blocks[1].text, "Existing block 1");
        } else {
            panic!("Expected Blocks variant");
        }
    }

    #[test]
    fn test_api_key_does_not_modify_prompt() {
        let strategy = ApiKeyStrategy::new("sk-ant-api-test");

        let existing = SystemPrompt::text("User prompt");
        let result = strategy.prepare_system_prompt(Some(existing.clone()));

        // API key should pass through without modification
        match result {
            Some(SystemPrompt::Text(text)) => assert_eq!(text, "User prompt"),
            _ => panic!("API key should return text prompt unchanged"),
        }
    }
}

// =============================================================================
// Test 3: Request Metadata Generation (OAuth)
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

    #[test]
    fn test_oauth_strategy_generates_metadata() {
        let cred = OAuthCredential {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        };
        let strategy = OAuthStrategy::new(cred);

        let metadata = strategy.prepare_metadata();
        assert!(metadata.is_some(), "OAuth should generate metadata");
    }

    #[test]
    fn test_api_key_no_metadata() {
        let strategy = ApiKeyStrategy::new("test-key");

        let metadata = strategy.prepare_metadata();
        assert!(metadata.is_none(), "API key should not generate metadata");
    }
}

// =============================================================================
// Test 4: Request Preparation (Full Flow)
// =============================================================================

mod request_preparation_tests {
    use super::*;

    fn create_test_oauth_strategy() -> OAuthStrategy {
        let cred = OAuthCredential {
            access_token: "test-token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        };
        OAuthStrategy::new(cred)
    }

    #[test]
    fn test_prepare_request_adds_system_and_metadata() {
        let strategy = create_test_oauth_strategy();

        let request =
            CreateMessageRequest::new("claude-sonnet-4-5-20250929", vec![Message::user("Hello")]);

        let prepared = strategy.prepare_request(request);

        // Check system prompt was added
        assert!(prepared.system.is_some(), "System prompt should be added");

        // Check metadata was added
        assert!(prepared.metadata.is_some(), "Metadata should be added");
    }

    #[test]
    fn test_prepare_request_preserves_existing_metadata() {
        let strategy = create_test_oauth_strategy();

        let custom_metadata = RequestMetadata {
            user_id: Some("custom-user-id".to_string()),
            extra: Default::default(),
        };

        let request =
            CreateMessageRequest::new("claude-sonnet-4-5-20250929", vec![Message::user("Hello")])
                .with_metadata(custom_metadata);

        let prepared = strategy.prepare_request(request);

        // Custom metadata should be preserved
        let metadata = prepared.metadata.unwrap();
        assert_eq!(metadata.user_id, Some("custom-user-id".to_string()));
    }

    #[test]
    fn test_api_key_minimal_request() {
        let strategy = ApiKeyStrategy::new("test-key");

        let request =
            CreateMessageRequest::new("claude-sonnet-4-5-20250929", vec![Message::user("Hello")]);

        let prepared = strategy.prepare_request(request);

        // API key should not add metadata
        assert!(prepared.metadata.is_none());
        // System prompt should be unchanged (None remains None)
        assert!(prepared.system.is_none());
    }
}

// =============================================================================
// Test 5: Tool Registry and Definitions
// =============================================================================

mod tool_registry_tests {
    use super::*;

    #[test]
    fn test_default_tools_registered() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);

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

        // Web tools (WebSearch is now a built-in API, not a custom tool)
        assert!(
            registry.contains("WebFetch"),
            "WebFetch tool should be registered"
        );

        // Notebook tools
        assert!(
            registry.contains("NotebookEdit"),
            "NotebookEdit tool should be registered"
        );
    }

    #[test]
    fn test_tool_access_filtering() {
        // Only allow Read and Write
        let registry = ToolRegistry::default_tools(&ToolAccess::only(["Read", "Write"]), None);

        assert!(registry.contains("Read"));
        assert!(registry.contains("Write"));
        assert!(!registry.contains("Bash"));
        assert!(!registry.contains("Grep"));
    }

    #[test]
    fn test_tool_access_except() {
        // Deny Bash only
        let registry = ToolRegistry::default_tools(&ToolAccess::except(["Bash"]), None);

        assert!(registry.contains("Read"));
        assert!(registry.contains("Write"));
        assert!(!registry.contains("Bash"));
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);

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
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);
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
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);
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
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);
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
// Test 6: OAuth Config Environment Variables
// =============================================================================

mod oauth_config_env_tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = OAuthConfig::default();

        assert!(config.system_prompt.contains("Claude Code"));
        assert_eq!(config.beta_flags.len(), 3);
        assert!(config.user_agent.contains("claude-cli"));
        assert_eq!(config.app_identifier, "cli");
        assert!(config.url_params.contains_key("beta"));
    }

    #[test]
    fn test_builder_overrides() {
        let config = OAuthConfig::builder()
            .system_prompt("Custom Prompt")
            .user_agent("test-agent/1.0")
            .app_identifier("test-app")
            .add_url_param("custom", "value")
            .add_header("X-Custom", "header")
            .build();

        assert_eq!(config.system_prompt, "Custom Prompt");
        assert_eq!(config.user_agent, "test-agent/1.0");
        assert_eq!(config.app_identifier, "test-app");
        assert!(config.url_params.contains_key("custom"));
        assert!(config.extra_headers.contains_key("X-Custom"));
    }

    #[test]
    fn test_beta_header_value_format() {
        let config = OAuthConfig::default();
        let header = config.beta_header_value();

        // Should be comma-separated
        assert!(header.contains(','));
        assert!(header.contains("claude-code-20250219"));
        assert!(header.contains("oauth-2025-04-20"));
    }
}

// =============================================================================
// Test 7: Strategy Pattern Completeness
// =============================================================================

mod strategy_completeness_tests {
    use super::*;

    #[test]
    fn test_strategy_names() {
        let api_strategy = ApiKeyStrategy::new("test");
        assert_eq!(api_strategy.name(), "api_key");

        let oauth_cred = OAuthCredential {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        };
        let oauth_strategy = OAuthStrategy::new(oauth_cred);
        assert_eq!(oauth_strategy.name(), "oauth");
    }

    #[test]
    fn test_api_key_vs_oauth_headers() {
        let api_strategy = ApiKeyStrategy::new("sk-ant-api-test");
        let (api_header_name, _) = api_strategy.auth_header();
        assert_eq!(api_header_name, "x-api-key");
        assert!(api_strategy.extra_headers().is_empty());

        let oauth_cred = OAuthCredential {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        };
        let oauth_strategy = OAuthStrategy::new(oauth_cred);
        let (oauth_header_name, _) = oauth_strategy.auth_header();
        assert_eq!(oauth_header_name, "Authorization");
        assert!(!oauth_strategy.extra_headers().is_empty());
    }
}

// =============================================================================
// Test 8: Tool Execution (Local Tests)
// =============================================================================

mod tool_execution_tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::fs;

    #[tokio::test]
    async fn test_read_tool_execution() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "Hello, World!\nLine 2")
            .await
            .unwrap();

        let registry =
            ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));

        let result = registry
            .execute(
                "Read",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap()
                }),
            )
            .await;

        match result {
            claude_agent::tools::ToolResult::Success(content) => {
                assert!(content.contains("Hello, World!"));
                assert!(content.contains("Line 2"));
            }
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_write_tool_execution() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let registry =
            ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));

        let result = registry
            .execute(
                "Write",
                serde_json::json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "New content"
                }),
            )
            .await;

        assert!(!result.is_error());
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

        let registry =
            ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));

        let result = registry
            .execute(
                "Glob",
                serde_json::json!({
                    "pattern": "*.txt",
                    "path": dir.path().to_str().unwrap()
                }),
            )
            .await;

        match result {
            claude_agent::tools::ToolResult::Success(content) => {
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

        let registry =
            ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));

        let result = registry
            .execute(
                "Grep",
                serde_json::json!({
                    "pattern": "World",
                    "path": dir.path().to_str().unwrap()
                }),
            )
            .await;

        match result {
            claude_agent::tools::ToolResult::Success(content) => {
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

        let registry =
            ToolRegistry::default_tools(&ToolAccess::All, Some(dir.path().to_path_buf()));

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
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);

        let result = registry
            .execute("NonexistentTool", serde_json::json!({}))
            .await;

        assert!(result.is_error());
        match result {
            claude_agent::tools::ToolResult::Error(msg) => {
                assert!(msg.contains("Unknown tool"));
            }
            _ => panic!("Expected error"),
        }
    }
}

// =============================================================================
// Test 9: Client Builder Integration
// =============================================================================

mod client_builder_tests {
    use super::*;

    #[test]
    fn test_builder_with_api_key() {
        // This should not fail even without network
        let _builder = Client::builder().api_key("sk-ant-api-test-key");
        // Just verify builder works (actual build would need valid credentials)
    }

    #[test]
    fn test_builder_with_oauth_config() {
        let _builder = Client::builder()
            .add_beta_flag("custom-flag")
            .user_agent("test-agent/1.0")
            .claude_code_prompt("Custom prompt");
    }

    #[test]
    fn test_builder_model_and_tokens() {
        let _builder = Client::builder()
            .api_key("test")
            .model("claude-opus-4")
            .max_tokens(16384);
    }
}

// =============================================================================
// Test 10: Progressive Disclosure - Tool Definitions
// =============================================================================

mod progressive_disclosure_tests {
    use super::*;

    #[test]
    fn test_tool_descriptions_are_concise() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);

        for def in registry.definitions() {
            // Tool descriptions should be useful but not overwhelming
            let desc_len = def.description.len();
            assert!(
                desc_len > 20,
                "Tool {} description too short: {} chars",
                def.name,
                desc_len
            );
            // Very long descriptions should be avoided for progressive disclosure
            assert!(
                desc_len < 2000,
                "Tool {} description may be too long for initial context: {} chars",
                def.name,
                desc_len
            );
        }
    }

    #[test]
    fn test_input_schemas_are_complete() {
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);

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
        let registry = ToolRegistry::default_tools(&ToolAccess::All, None);
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
// Test 11: Credential Expiry Handling
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
}
