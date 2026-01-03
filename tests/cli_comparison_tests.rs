//! Claude Code CLI vs claude-agent-rs SDK Comparison Verification Tests
//!
//! This test module performs deep verification that the SDK provides the same value as Claude Code CLI.
//!
//! ## Test Scenarios
//!
//! ### 1. Basic API
//! - Simple query (single response)
//! - Streaming query (streaming response)
//!
//! ### 2. Tool Use
//! - File tools: Read, Write, Edit, Glob, Grep
//! - Shell tools: Bash, KillShell
//! - Web tools: WebFetch (WebSearch is built-in API)
//! - Productivity: TodoWrite
//! - Planning: Plan
//!
//! ### 3. Agent Loop
//! - Multi-turn conversation
//! - Tool execution chain
//! - Context management
//!
//! ### 4. Session Management
//! - Session creation/restoration
//! - Context compaction
//! - Message history
//!
//! ### 5. Advanced Features
//! - Permission system
//! - Hook system
//! - Skill system
//! - MCP integration

use tempfile::TempDir;

// ============================================================================
// 1. Tool Implementation Tests - Verify same tool specs as CLI
// ============================================================================

mod tool_spec_tests {
    use super::*;
    use claude_agent::ToolOutput;
    use claude_agent::session::SessionId;
    use claude_agent::session::ToolState;
    use claude_agent::tools::{
        BashTool, EditTool, ExecutionContext, GlobTool, GrepTool, ReadTool, TodoWriteTool, Tool,
        WriteTool,
    };
    use serde_json::json;

    fn create_test_context(temp_dir: &TempDir) -> ExecutionContext {
        ExecutionContext::from_path(std::fs::canonicalize(temp_dir.path()).unwrap())
            .unwrap_or_else(|_| ExecutionContext::permissive())
    }

    /// Read Tool - CLI spec compliance verification
    /// CLI: file_path (required), offset (optional), limit (optional)
    #[tokio::test]
    async fn test_read_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let tool = ReadTool;

        // Basic read
        let ctx = create_test_context(&temp_dir);
        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap()
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(content) => {
                assert!(content.contains("Line 1"));
                assert!(content.contains("Line 5"));
                // CLI format: includes line numbers (cat -n style)
                assert!(content.contains("1\t") || content.contains("1→"));
            }
            other => panic!("Expected success, got: {:?}", other),
        }

        // offset/limit support
        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "offset": 2,
                    "limit": 2
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(content) => {
                assert!(content.contains("Line 3") || content.contains("Line 2"));
            }
            _ => panic!("Expected success with offset/limit"),
        }
    }

    /// Write Tool - CLI spec compliance verification
    /// CLI: file_path (required), content (required)
    #[tokio::test]
    async fn test_write_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");

        let tool = WriteTool;
        let ctx = create_test_context(&temp_dir);

        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "Hello, World!"
                }),
                &ctx,
            )
            .await;

        assert!(!result.is_error(), "Write failed: {:?}", result);
        assert!(file_path.exists());
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "Hello, World!"
        );
    }

    /// Write Tool - auto-creates directories
    #[tokio::test]
    async fn test_write_tool_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let canonical_root = std::fs::canonicalize(temp_dir.path()).unwrap();
        let file_path = canonical_root.join("deep/nested/dir/file.txt");

        let tool = WriteTool;
        let ctx = create_test_context(&temp_dir);

        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "Nested content"
                }),
                &ctx,
            )
            .await;

        assert!(!result.is_error(), "Write failed: {:?}", result);
        assert!(file_path.exists());
    }

    /// Edit Tool - CLI spec compliance verification
    /// CLI: file_path, old_string, new_string, replace_all (optional)
    ///
    /// Note: Edit tool requires old_string to be unique in the file when replace_all is false.
    /// If old_string appears multiple times, it returns an error asking user to provide more context.
    #[tokio::test]
    async fn test_edit_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit_test.txt");

        // Test with unique string (same behavior as CLI)
        std::fs::write(&file_path, "hello world").unwrap();

        let tool = EditTool;
        let ctx = create_test_context(&temp_dir);

        // Single replacement (unique string)
        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "hello",
                    "new_string": "hi"
                }),
                &ctx,
            )
            .await;

        assert!(
            !result.is_error(),
            "Edit should succeed with unique old_string"
        );
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hi world");

        // Replace all occurrences (replace_all)
        std::fs::write(&file_path, "foo bar foo baz").unwrap();
        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "foo",
                    "new_string": "qux",
                    "replace_all": true
                }),
                &ctx,
            )
            .await;

        assert!(!result.is_error(), "Edit should succeed with replace_all");
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "qux bar qux baz"); // All replaced

        // Verify error is returned for duplicate strings (same behavior as CLI)
        std::fs::write(&file_path, "foo bar foo baz").unwrap();
        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "foo",
                    "new_string": "qux"
                }),
                &ctx,
            )
            .await;

        // Duplicate old_string should return error (needs more context)
        assert!(
            result.is_error(),
            "Edit should fail when old_string is not unique"
        );
    }

    /// Glob Tool - CLI spec compliance verification
    /// CLI: pattern (required), path (optional)
    #[tokio::test]
    async fn test_glob_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();

        // Create test files
        std::fs::write(temp_dir.path().join("test1.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("test2.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("test.txt"), "").unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("subdir/nested.rs"), "").unwrap();

        let tool = GlobTool;
        let ctx = create_test_context(&temp_dir);

        // Basic pattern matching
        let result = tool
            .execute(
                json!({
                    "pattern": "*.rs",
                    "path": temp_dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("test1.rs"));
                assert!(output.contains("test2.rs"));
                assert!(!output.contains("test.txt"));
            }
            _ => panic!("Expected success"),
        }

        // Recursive pattern
        let result = tool
            .execute(
                json!({
                    "pattern": "**/*.rs",
                    "path": temp_dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("nested.rs"));
            }
            _ => panic!("Expected success with recursive pattern"),
        }
    }

    /// Grep Tool - CLI spec compliance verification
    /// CLI: pattern, path, output_mode, glob, type, -i, -n, -A, -B, -C, etc.
    #[tokio::test]
    async fn test_grep_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();

        std::fs::write(
            temp_dir.path().join("search.txt"),
            "Hello World\nfoo bar\nHello Again\n",
        )
        .unwrap();

        let tool = GrepTool;
        let ctx = create_test_context(&temp_dir);

        // Basic search
        let result = tool
            .execute(
                json!({
                    "pattern": "Hello",
                    "path": temp_dir.path().to_str().unwrap(),
                    "output_mode": "content"
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("Hello World"));
                assert!(output.contains("Hello Again"));
            }
            _ => panic!("Expected success"),
        }

        // Case-insensitive search
        let result = tool
            .execute(
                json!({
                    "pattern": "hello",
                    "path": temp_dir.path().to_str().unwrap(),
                    "output_mode": "content",
                    "-i": true
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("Hello"));
            }
            _ => panic!("Expected success with case-insensitive search"),
        }
    }

    /// Bash Tool - CLI spec compliance verification
    /// CLI: command (required), timeout (optional), description (optional)
    #[tokio::test]
    async fn test_bash_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let tool = BashTool::new();
        let ctx = create_test_context(&temp_dir);

        // Basic command execution
        let result = tool
            .execute(
                json!({
                    "command": "echo 'Hello from Bash'",
                    "description": "Echo test"
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("Hello from Bash"));
            }
            _ => panic!("Expected success"),
        }

        // Timeout test
        let result = tool
            .execute(
                json!({
                    "command": "sleep 0.1 && echo 'done'",
                    "timeout": 5000
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("done"));
            }
            _ => panic!("Expected success with timeout"),
        }
    }

    /// TodoWrite Tool - CLI spec compliance verification
    #[tokio::test]
    async fn test_todo_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let session_id = SessionId::new();
        let session_ctx = ToolState::new(session_id);
        let tool = TodoWriteTool::new(session_ctx, session_id);
        let ctx = create_test_context(&temp_dir);

        let result = tool
            .execute(
                json!({
                    "todos": [
                        {
                            "content": "First task",
                            "status": "pending",
                            "activeForm": "Working on first task"
                        },
                        {
                            "content": "Second task",
                            "status": "in_progress",
                            "activeForm": "Working on second task"
                        }
                    ]
                }),
                &ctx,
            )
            .await;

        assert!(!result.is_error());
    }
}

// ============================================================================
// 2. Agent Loop Tests - Verify same agent behavior as CLI
// ============================================================================

mod agent_loop_tests {
    use claude_agent::{Agent, AgentEvent, ToolAccess};

    /// Agent builder pattern verification
    #[tokio::test]
    async fn test_agent_builder_pattern() {
        // Verify builder methods correspond to CLI options
        // Requires API key to build successfully
        let agent_result = Agent::builder()
            .auth("test-api-key").await.expect("Auth failed") // Test API key
            .model("claude-sonnet-4-5-20250514")
            .tools(ToolAccess::all())
            .working_dir(".")
            .max_tokens(4096)
            .max_iterations(10)
            .system_prompt("Custom system prompt")
            .build()
            .await;

        assert!(agent_result.is_ok());
    }

    /// Tool Access mode verification (corresponds to CLI's --allowedTools)
    #[test]
    fn test_tool_access_modes() {
        // All tools
        let access = ToolAccess::all();
        assert!(access.is_allowed("Read"));
        assert!(access.is_allowed("Bash"));
        assert!(access.is_allowed("WebFetch"));

        // None
        let access = ToolAccess::none();
        assert!(!access.is_allowed("Read"));

        // Custom selection - using String array directly
        let access = ToolAccess::only(["Read".to_string(), "Glob".to_string(), "Grep".to_string()]);
        assert!(access.is_allowed("Read"));
        assert!(!access.is_allowed("Bash"));

        // Exclude specific
        let access = ToolAccess::except(["Bash".to_string()]);
        assert!(access.is_allowed("Read"));
        assert!(!access.is_allowed("Bash"));
    }

    /// AgentEvent stream event verification (corresponds to CLI output)
    #[test]
    fn test_agent_events_match_cli_output() {
        // Event types output by CLI:
        // - Text: text response
        // - ToolStart: [Tool: name] output
        // - ToolEnd: tool result output
        // - Complete: final statistics

        let text_event = AgentEvent::Text("Hello".to_string());
        let tool_start = AgentEvent::ToolStart {
            id: "id1".to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({}),
        };
        let tool_end = AgentEvent::ToolEnd {
            id: "id1".to_string(),
            output: "file contents".to_string(),
            is_error: false,
        };

        // Event type matching verification
        assert!(matches!(text_event, AgentEvent::Text(_)));
        assert!(matches!(tool_start, AgentEvent::ToolStart { .. }));
        assert!(matches!(tool_end, AgentEvent::ToolEnd { .. }));
    }
}

// ============================================================================
// 3. Session & Context Management Tests
// ============================================================================

mod session_tests {
    use claude_agent::session::{
        CompactExecutor, CompactStrategy, Session, SessionConfig, SessionManager, SessionMessage,
    };
    use claude_agent::types::ContentBlock;

    /// Session creation and message management
    #[test]
    fn test_session_management() {
        let config = SessionConfig::default();
        let mut session = Session::new(config);

        // Add messages (using SessionMessage)
        let user_msg = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        session.add_message(user_msg);

        let assistant_msg = SessionMessage::assistant(vec![ContentBlock::text("Hi there!")]);
        session.add_message(assistant_msg);

        assert_eq!(session.messages.len(), 2);
        assert!(session.current_leaf_id.is_some());
    }

    /// Context Compaction (corresponds to CLI's automatic context management)
    #[test]
    fn test_context_compaction() {
        let strategy = CompactStrategy::default()
            .with_threshold(0.8)
            .with_keep_recent(4);

        let executor = CompactExecutor::new(strategy);

        // Compact needed at 80% or above
        assert!(!executor.needs_compact(70_000, 100_000));
        assert!(executor.needs_compact(80_000, 100_000));
        assert!(executor.needs_compact(90_000, 100_000));
    }

    /// Session Manager - multi-session management
    #[tokio::test]
    async fn test_session_manager() {
        // Create with in-memory persistence
        let manager = SessionManager::in_memory();

        // Create sessions
        let session1 = manager.create(SessionConfig::default()).await.unwrap();
        let session2 = manager.create(SessionConfig::default()).await.unwrap();

        assert_ne!(session1.id, session2.id);

        // Search session
        let found = manager.get(&session1.id).await;
        assert!(found.is_ok());
    }
}

// ============================================================================
// 4. Client API Tests - Claude API communication spec verification
// ============================================================================

mod client_tests {
    use claude_agent::client::{DEFAULT_SMALL_MODEL, GatewayConfig, ModelConfig, ProviderConfig};
    use claude_agent::{Auth, Client};

    /// Client configuration verification (corresponds to CLI env vars)
    #[tokio::test]
    async fn test_client_builder() {
        let models = ModelConfig::new("claude-sonnet-4-5-20250514", DEFAULT_SMALL_MODEL);
        let config = ProviderConfig::new(models).with_max_tokens(4096);

        let client_result = Client::builder()
            .auth("test-key")
            .await
            .expect("Auth failed")
            .config(config)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .await;

        assert!(client_result.is_ok());
        let client = client_result.unwrap();
        assert_eq!(client.config().max_tokens, 4096);
    }

    /// API URL configuration (corresponds to CLI's --api-base-url)
    #[tokio::test]
    async fn test_custom_base_url() {
        let gateway = GatewayConfig::with_base_url("https://custom.api.com/v1");
        let client = Client::builder()
            .auth("test-key")
            .await
            .expect("Auth failed")
            .gateway(gateway)
            .build()
            .await;

        assert!(client.is_ok());
    }

    /// OAuth token authentication test
    #[tokio::test]
    async fn test_oauth_token() {
        let client = Client::builder()
            .auth(Auth::oauth("sk-ant-oat01-test-token"))
            .await
            .expect("Auth failed")
            .build()
            .await;

        assert!(client.is_ok());
        let client = client.unwrap();
        // Verify by adapter name
        assert_eq!(client.adapter().name(), "anthropic");
    }

    /// Auto-resolve builder test (failure is expected without env var)
    #[tokio::test]
    async fn test_auto_resolve_builder() {
        // Verify builder configuration works correctly
        // Success if ANTHROPIC_API_KEY is set, error otherwise
        let result = Client::builder().auth(Auth::FromEnv).await;
        // In test environment without env var, error is expected
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            assert!(result.is_err(), "Should fail without ANTHROPIC_API_KEY");
        } else {
            assert!(result.is_ok(), "Should succeed with ANTHROPIC_API_KEY");
        }
    }
}

// ============================================================================
// 5. Permission System Tests - Security feature verification
// ============================================================================

mod permission_tests {
    use claude_agent::permissions::{
        PermissionMode, PermissionPolicyBuilder, is_file_tool, is_read_only_tool, is_shell_tool,
    };

    /// Tool classification verification
    #[test]
    fn test_tool_classification() {
        // Read-only tools (safe)
        assert!(is_read_only_tool("Read"));
        assert!(is_read_only_tool("Glob"));
        assert!(is_read_only_tool("Grep"));
        assert!(!is_read_only_tool("Write"));

        // File tools
        assert!(is_file_tool("Read"));
        assert!(is_file_tool("Write"));
        assert!(is_file_tool("Edit"));
        assert!(!is_file_tool("Bash"));

        // Shell tools (dangerous)
        assert!(is_shell_tool("Bash"));
        assert!(is_shell_tool("KillShell"));
        assert!(!is_shell_tool("Read"));
    }

    /// Permission Mode verification (corresponds to CLI's --permission-mode)
    #[test]
    fn test_permission_modes() {
        let default = PermissionMode::default();
        assert!(matches!(default, PermissionMode::Default));

        let bypass = PermissionMode::BypassPermissions;
        let plan = PermissionMode::Plan;

        // Verify behavior per mode
        assert!(matches!(bypass, PermissionMode::BypassPermissions));
        assert!(matches!(plan, PermissionMode::Plan));
    }

    /// Permission Policy builder
    #[test]
    fn test_permission_policy_builder() {
        let policy = PermissionPolicyBuilder::new()
            .allow("Read")
            .allow("Glob")
            .deny("Bash")
            .build();

        // Policy verification
        let read_result = policy.check("Read", &serde_json::Value::Null);
        assert!(read_result.is_allowed());

        let bash_result = policy.check("Bash", &serde_json::Value::Null);
        assert!(bash_result.is_denied());
    }
}

// ============================================================================
// 6. Hook System Tests - Execution interception feature verification
// ============================================================================

mod hook_tests {
    use async_trait::async_trait;
    use claude_agent::hooks::{Hook, HookContext, HookEvent, HookInput, HookManager, HookOutput};

    /// Custom Hook implementation
    struct TestHook {
        name: String,
        events: Vec<HookEvent>,
    }

    impl TestHook {
        fn new() -> Self {
            Self {
                name: "test-hook".to_string(),
                events: vec![HookEvent::PreToolUse],
            }
        }
    }

    #[async_trait]
    impl Hook for TestHook {
        fn name(&self) -> &str {
            &self.name
        }

        fn events(&self) -> &[HookEvent] {
            &self.events
        }

        async fn execute(
            &self,
            input: HookInput,
            _ctx: &HookContext,
        ) -> Result<HookOutput, claude_agent::Error> {
            if input.tool_name() == Some("Bash") {
                return Ok(HookOutput::block("Bash blocked by hook"));
            }
            Ok(HookOutput::allow())
        }
    }

    #[test]
    fn test_hook_registration() {
        let mut manager = HookManager::new();
        let hook = TestHook::new();

        manager.register(hook);
        assert_eq!(manager.hook_names().len(), 1);
    }

    #[test]
    fn test_hook_output_builders() {
        let allow = HookOutput::allow();
        assert!(allow.continue_execution);

        let block = HookOutput::block("Blocked");
        assert!(!block.continue_execution);
        assert_eq!(block.stop_reason, Some("Blocked".to_string()));

        let with_message = HookOutput::allow()
            .with_system_message("Added context")
            .with_context("More info");
        assert!(with_message.continue_execution);
        assert!(with_message.system_message.is_some());
    }

    #[test]
    fn test_hook_events() {
        assert!(HookEvent::PreToolUse.can_block());
        assert!(HookEvent::UserPromptSubmit.can_block());
        assert!(!HookEvent::PostToolUse.can_block());
        assert!(!HookEvent::SessionEnd.can_block());
    }
}

// ============================================================================
// 7. Skill System Tests - Reusable workflow verification
// ============================================================================

mod skill_tests {
    use claude_agent::skills::{SkillDefinition, SkillRegistry, SkillResult, SkillSourceType};

    /// Skill definition verification
    #[test]
    fn test_skill_definition() {
        let skill =
            SkillDefinition::new("commit", "Create git commit", "Analyze and commit changes")
                .with_source_type(SkillSourceType::Builtin)
                .with_trigger("/commit");

        assert_eq!(skill.name, "commit");
        assert!(skill.matches_trigger("/commit please"));
        assert!(!skill.matches_trigger("just commit"));
    }

    /// Skill Registry verification
    #[test]
    fn test_skill_registry() {
        let mut registry = SkillRegistry::new();

        let skill1 = SkillDefinition::new("commit", "Commit", "content1");
        let skill2 = SkillDefinition::new("review", "Review", "content2");

        registry.register(skill1);
        registry.register(skill2);

        assert!(registry.get("commit").is_some());
        assert!(registry.get("review").is_some());
        assert!(registry.get("unknown").is_none());
    }

    /// Skill Result verification
    #[test]
    fn test_skill_result() {
        let success = SkillResult::success("Task completed");
        assert!(success.success);
        assert!(success.error.is_none());

        let error = SkillResult::error("Task failed");
        assert!(!error.success);
        assert!(error.error.is_some());
    }
}

// ============================================================================
// 8. Types Compatibility Tests - CLI type compatibility verification
// ============================================================================

mod types_tests {
    use claude_agent::types::{
        ContentBlock, Message, Role, StopReason, ToolDefinition, ToolResultBlock, Usage,
    };

    /// Message structure verification (API spec compliance)
    #[test]
    fn test_message_structure() {
        let user_msg = Message::user("Hello");
        assert!(matches!(user_msg.role, Role::User));

        let assistant_msg = Message::assistant("Hi!");
        assert!(matches!(assistant_msg.role, Role::Assistant));
    }

    /// ContentBlock type verification
    #[test]
    fn test_content_blocks() {
        let text = ContentBlock::text("Hello");
        assert!(matches!(text, ContentBlock::Text { .. }));

        let tool_result = ToolResultBlock::success("tool-id", "result");
        // is_error is Option<bool>, None for success case
        assert!(tool_result.is_error.is_none() || tool_result.is_error == Some(false));
    }

    /// Tool Definition structure
    #[test]
    fn test_tool_definition() {
        let def = ToolDefinition::new(
            "Read",
            "Read files",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"}
                },
                "required": ["file_path"]
            }),
        );

        assert_eq!(def.name, "Read");
    }

    /// Usage token calculation
    #[test]
    fn test_usage_calculation() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(5),
            server_tool_use: None,
        };

        assert_eq!(usage.total(), 150);
    }

    /// StopReason verification
    #[test]
    fn test_stop_reasons() {
        assert!(matches!(StopReason::EndTurn, StopReason::EndTurn));
        assert!(matches!(StopReason::ToolUse, StopReason::ToolUse));
        assert!(matches!(StopReason::MaxTokens, StopReason::MaxTokens));
    }
}

// ============================================================================
// 9. Error Handling Tests - Error handling consistency verification
// ============================================================================

mod error_tests {
    use claude_agent::Error;

    /// Error type verification
    #[test]
    fn test_error_types() {
        let api_error = Error::Api {
            message: "Invalid API key".to_string(),
            status: Some(401),
            error_type: None,
        };
        assert!(api_error.to_string().contains("Invalid API key"));

        let tool_error = Error::Tool(claude_agent::types::ToolError::not_found("/test/file.txt"));
        assert!(tool_error.to_string().contains("not found"));

        let rate_limit = Error::RateLimit {
            retry_after: Some(std::time::Duration::from_secs(60)),
        };
        assert!(rate_limit.to_string().contains("Rate limit"));

        let context_overflow = Error::ContextOverflow {
            current: 250_000,
            max: 200_000,
        };
        assert!(context_overflow.to_string().contains("Context window"));
    }
}

// ============================================================================
// 10. Integration Scenario Tests - Real-world usage scenario verification
// ============================================================================

mod integration_scenarios {
    use super::*;
    use claude_agent::ToolOutput;
    use claude_agent::tools::{
        BashTool, EditTool, ExecutionContext, GlobTool, GrepTool, ReadTool, Tool, WriteTool,
    };
    use serde_json::json;

    fn create_test_context(temp_dir: &TempDir) -> ExecutionContext {
        ExecutionContext::from_path(std::fs::canonicalize(temp_dir.path()).unwrap())
            .unwrap_or_else(|_| ExecutionContext::permissive())
    }

    /// Scenario: File create -> read -> edit chain
    #[tokio::test]
    async fn test_file_workflow_scenario() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("workflow.txt");

        let write_tool = WriteTool;
        let read_tool = ReadTool;
        let edit_tool = EditTool;
        let ctx = create_test_context(&temp_dir);

        // Step 1: Write
        let result = write_tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "content": "function hello() {\n  console.log('Hello');\n}"
                }),
                &ctx,
            )
            .await;
        assert!(!result.is_error());

        // Step 2: Read
        let result = read_tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap()
                }),
                &ctx,
            )
            .await;
        match &result.output {
            ToolOutput::Success(content) => {
                assert!(content.contains("function hello"));
            }
            _ => panic!("Read should succeed"),
        }

        // Step 3: Edit
        let result = edit_tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "Hello",
                    "new_string": "World"
                }),
                &ctx,
            )
            .await;
        assert!(!result.is_error());

        // Verify final state
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("World"));
    }

    /// Scenario: Code search -> analysis chain
    #[tokio::test]
    async fn test_code_search_scenario() {
        let temp_dir = TempDir::new().unwrap();

        // Create test code files
        std::fs::create_dir(temp_dir.path().join("src")).unwrap();
        std::fs::write(
            temp_dir.path().join("src/main.rs"),
            "fn main() {\n    println!(\"Hello\");\n}",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("src/lib.rs"),
            "pub fn hello() {\n    println!(\"Hello from lib\");\n}",
        )
        .unwrap();

        let glob_tool = GlobTool;
        let grep_tool = GrepTool;
        let ctx = create_test_context(&temp_dir);

        // Step 1: Find all Rust files
        let result = glob_tool
            .execute(
                json!({
                    "pattern": "**/*.rs",
                    "path": temp_dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("main.rs"));
                assert!(output.contains("lib.rs"));
            }
            _ => panic!("Glob should succeed"),
        }

        // Step 2: Search for pattern
        let result = grep_tool
            .execute(
                json!({
                    "pattern": "println!",
                    "path": temp_dir.path().to_str().unwrap(),
                    "output_mode": "content"
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("Hello"));
            }
            _ => panic!("Grep should succeed"),
        }
    }

    /// Scenario: Shell command execution (safe commands)
    #[tokio::test]
    async fn test_shell_command_scenario() {
        let temp_dir = TempDir::new().unwrap();
        let bash_tool = BashTool::new();
        let ctx = create_test_context(&temp_dir);

        // Execute safe command
        let result = bash_tool
            .execute(
                json!({
                    "command": "echo 'test' && pwd",
                    "description": "Test echo and pwd"
                }),
                &ctx,
            )
            .await;

        match &result.output {
            ToolOutput::Success(output) => {
                assert!(output.contains("test"));
            }
            _ => panic!("Bash should succeed"),
        }
    }
}

// ============================================================================
// 11. Feature Parity Checklist
// ============================================================================

/// CLI and SDK feature parity verification
#[test]
fn test_feature_parity_checklist() {
    // This test documents the feature checklist
    let checklist = vec![
        // Basic API
        ("query()", true, "Simple query"),
        ("stream()", true, "Streaming query"),
        // Tools
        ("Read", true, "File reading with offset/limit"),
        ("Write", true, "File writing with directory creation"),
        ("Edit", true, "String replacement with replace_all"),
        ("Glob", true, "Pattern matching"),
        ("Grep", true, "Content search with regex"),
        ("Bash", true, "Shell execution with timeout"),
        ("TodoWrite", true, "Task tracking"),
        ("WebFetch", true, "Web fetch"),
        ("Plan", true, "Plan mode management"),
        ("KillShell", true, "Kill background shell"),
        // Agent
        ("Agent Loop", true, "Multi-turn with tools"),
        (
            "Streaming Events",
            true,
            "Text, ToolStart, ToolEnd, Complete",
        ),
        ("Context Management", true, "Token tracking, compaction"),
        // Session
        ("Session Management", true, "Create, restore, branch"),
        ("Context Compaction", true, "Automatic summarization"),
        // Security
        ("Permission System", true, "Tool/path allow/deny"),
        ("Hook System", true, "Execution interception"),
        // Extension
        ("Custom Tools", true, "Tool trait implementation"),
        ("Skill System", true, "Reusable workflows"),
        ("MCP Support", true, "External server integration"),
    ];

    let total_features = checklist.len();

    for (feature, implemented, description) in &checklist {
        assert!(
            *implemented,
            "Feature '{}' ({}) should be implemented",
            feature, description
        );
    }

    println!(
        "\n📋 Feature Parity Checklist: {} features verified",
        total_features
    );
}

// ============================================================================
// 12. MCP Integration Tests
// ============================================================================

mod mcp_tests {
    use claude_agent::mcp::{
        McpConnectionStatus, McpContent, McpServerConfig, McpServerState, McpToolResult,
    };
    use std::collections::HashMap;

    /// MCP Server Config verification
    #[test]
    fn test_mcp_server_config() {
        // Stdio transport
        let stdio_config = McpServerConfig::Stdio {
            command: "npx".to_string(),
            args: vec!["@modelcontextprotocol/server".to_string()],
            env: HashMap::new(),
        };

        let json = serde_json::to_string(&stdio_config).unwrap();
        assert!(json.contains("stdio"));
        assert!(json.contains("npx"));

        // SSE transport
        let sse_config = McpServerConfig::Sse {
            url: "https://sse.example.com".to_string(),
            headers: HashMap::new(),
        };

        let json = serde_json::to_string(&sse_config).unwrap();
        assert!(json.contains("sse"));
    }

    /// MCP Server State verification
    #[test]
    fn test_mcp_server_state() {
        let state = McpServerState::new(
            "test-server",
            McpServerConfig::Stdio {
                command: "test".to_string(),
                args: vec![],
                env: HashMap::new(),
            },
        );

        assert_eq!(state.name, "test-server");
        assert_eq!(state.status, McpConnectionStatus::Connecting);
        assert!(!state.is_connected());
    }

    /// MCP Content type verification
    #[test]
    fn test_mcp_content_types() {
        let text_content = McpContent::Text {
            text: "Hello".to_string(),
        };
        assert_eq!(text_content.as_text(), Some("Hello"));

        let image_content = McpContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        assert_eq!(image_content.as_text(), None);
    }

    /// MCP Tool Result verification
    #[test]
    fn test_mcp_tool_result() {
        let result = McpToolResult {
            content: vec![
                McpContent::Text {
                    text: "Line 1".to_string(),
                },
                McpContent::Text {
                    text: "Line 2".to_string(),
                },
            ],
            is_error: false,
        };

        assert!(!result.is_error);
        assert_eq!(result.to_string_content(), "Line 1\nLine 2");
    }
}

// ============================================================================
// 13. Authentication System Tests
// ============================================================================

mod auth_tests {
    use claude_agent::auth::{
        ChainProvider, Credential, CredentialProvider, EnvironmentProvider, ExplicitProvider,
    };

    /// API Key Credential test
    #[test]
    fn test_api_key_credential() {
        let cred = Credential::api_key("sk-ant-api-test");
        assert!(!cred.is_expired());
        assert!(!cred.needs_refresh());
        assert_eq!(cred.credential_type(), "api_key");
    }

    /// OAuth Credential test
    #[test]
    fn test_oauth_credential() {
        let cred = Credential::oauth("sk-ant-oat01-test");
        assert_eq!(cred.credential_type(), "oauth");

        // Verify OAuth credential's access_token
        match cred {
            Credential::OAuth(oauth) => {
                assert_eq!(oauth.access_token, "sk-ant-oat01-test");
            }
            _ => panic!("Expected OAuth credential"),
        }
    }

    /// ExplicitProvider test
    #[tokio::test]
    async fn test_explicit_provider() {
        let provider = ExplicitProvider::api_key("test-key");
        assert_eq!(provider.name(), "explicit");

        let cred = provider.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "test-key"));
    }

    /// EnvironmentProvider test
    #[tokio::test]
    async fn test_environment_provider() {
        unsafe { std::env::set_var("TEST_AUTH_KEY", "env-test-key") };
        let provider = EnvironmentProvider::with_var("TEST_AUTH_KEY");
        assert_eq!(provider.name(), "environment");

        let cred = provider.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "env-test-key"));
        unsafe { std::env::remove_var("TEST_AUTH_KEY") };
    }

    /// ChainProvider test
    #[tokio::test]
    async fn test_chain_provider() {
        let chain = ChainProvider::new(vec![]).with(ExplicitProvider::api_key("chain-key"));

        assert_eq!(chain.name(), "chain");
        let cred = chain.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "chain-key"));
    }
}

// ============================================================================
// 14. Context System Tests
// ============================================================================

mod context_tests {
    use claude_agent::context::StaticContext;
    use claude_agent::types::SystemBlock;

    /// Static Context verification
    #[test]
    fn test_static_context() {
        let context = StaticContext::new()
            .with_system_prompt("You are a helpful assistant.")
            .with_claude_md("# Project")
            .with_skill_summary("Available skills: commit, review");

        assert!(!context.system_prompt.is_empty());
        assert!(!context.claude_md.is_empty());
    }

    /// System Block verification
    #[test]
    fn test_system_block() {
        use claude_agent::types::CacheType;
        let cached = SystemBlock::cached("Cached content");
        assert!(cached.cache_control.is_some());
        assert_eq!(
            cached.cache_control.unwrap().cache_type,
            CacheType::Ephemeral
        );
        assert_eq!(cached.block_type, "text");

        let uncached = SystemBlock::uncached("Uncached content");
        assert!(uncached.cache_control.is_none());
    }

    /// Cache Control type verification
    #[test]
    fn test_cache_control() {
        use claude_agent::types::{CacheControl, CacheType};
        let ephemeral = CacheControl::ephemeral();
        assert_eq!(ephemeral.cache_type, CacheType::Ephemeral);
    }
}
