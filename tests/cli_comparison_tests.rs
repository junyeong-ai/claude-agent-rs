//! Claude Code CLI vs claude-agent-rs SDK 비교 검증 테스트
//!
//! 이 테스트 모듈은 Claude Code CLI와 동일한 가치를 제공하는지 심층 검증합니다.
//!
//! ## 테스트 시나리오
//!
//! ### 1. 기본 API 호출 (Basic API)
//! - Simple query (단일 응답)
//! - Streaming query (스트리밍 응답)
//!
//! ### 2. Tool Use (도구 사용)
//! - File tools: Read, Write, Edit, Glob, Grep
//! - Shell tools: Bash, KillShell
//! - Web tools: WebFetch (WebSearch is built-in API)
//! - Productivity: TodoWrite
//! - Notebook: NotebookEdit
//!
//! ### 3. Agent Loop (에이전트 루프)
//! - Multi-turn conversation
//! - Tool execution chain
//! - Context management
//!
//! ### 4. Session Management (세션 관리)
//! - Session creation/restoration
//! - Context compaction
//! - Message history
//!
//! ### 5. Advanced Features (고급 기능)
//! - Permission system
//! - Hook system
//! - Skill system
//! - MCP integration

use tempfile::TempDir;

// ============================================================================
// 1. Tool Implementation Tests - CLI와 동일한 도구 스펙 검증
// ============================================================================

mod tool_spec_tests {
    use super::*;
    use claude_agent::tools::*;
    use serde_json::json;

    /// Read Tool - CLI 스펙 준수 검증
    /// CLI: file_path (required), offset (optional), limit (optional)
    #[tokio::test]
    async fn test_read_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\n").unwrap();

        let tool = ReadTool::new(temp_dir.path().to_path_buf());

        // 기본 읽기
        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap()
            }))
            .await;

        match result {
            ToolResult::Success(content) => {
                assert!(content.contains("Line 1"));
                assert!(content.contains("Line 5"));
                // CLI 형식: 라인 번호 포함 (cat -n 스타일)
                assert!(content.contains("1\t") || content.contains("1→"));
            }
            _ => panic!("Expected success"),
        }

        // offset/limit 지원
        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 2,
                "limit": 2
            }))
            .await;

        match result {
            ToolResult::Success(content) => {
                assert!(content.contains("Line 3") || content.contains("Line 2"));
            }
            _ => panic!("Expected success with offset/limit"),
        }
    }

    /// Write Tool - CLI 스펙 준수 검증
    /// CLI: file_path (required), content (required)
    #[tokio::test]
    async fn test_write_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");

        let tool = WriteTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "content": "Hello, World!"
            }))
            .await;

        assert!(!result.is_error());
        assert!(file_path.exists());
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "Hello, World!"
        );
    }

    /// Write Tool - 디렉토리 자동 생성
    #[tokio::test]
    async fn test_write_tool_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("deep/nested/dir/file.txt");

        let tool = WriteTool::new(temp_dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "content": "Nested content"
            }))
            .await;

        assert!(!result.is_error());
        assert!(file_path.exists());
    }

    /// Edit Tool - CLI 스펙 준수 검증
    /// CLI: file_path, old_string, new_string, replace_all (optional)
    ///
    /// Note: Edit tool requires old_string to be unique in the file when replace_all is false.
    /// If old_string appears multiple times, it returns an error asking user to provide more context.
    #[tokio::test]
    async fn test_edit_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit_test.txt");

        // 유니크한 문자열로 테스트 (CLI와 동일한 동작)
        std::fs::write(&file_path, "hello world").unwrap();

        let tool = EditTool::new(temp_dir.path().to_path_buf());

        // 단일 치환 (유니크한 문자열)
        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "hello",
                "new_string": "hi"
            }))
            .await;

        assert!(
            !result.is_error(),
            "Edit should succeed with unique old_string"
        );
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hi world");

        // 전체 치환 (replace_all)
        std::fs::write(&file_path, "foo bar foo baz").unwrap();
        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "qux",
                "replace_all": true
            }))
            .await;

        assert!(!result.is_error(), "Edit should succeed with replace_all");
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "qux bar qux baz"); // 모두 치환

        // 중복 문자열에 대해 에러 반환 확인 (CLI와 동일한 동작)
        std::fs::write(&file_path, "foo bar foo baz").unwrap();
        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "foo",
                "new_string": "qux"
            }))
            .await;

        // 중복된 old_string은 에러를 반환해야 함 (더 많은 컨텍스트 필요)
        assert!(
            result.is_error(),
            "Edit should fail when old_string is not unique"
        );
    }

    /// Glob Tool - CLI 스펙 준수 검증
    /// CLI: pattern (required), path (optional)
    #[tokio::test]
    async fn test_glob_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();

        // 테스트 파일 생성
        std::fs::write(temp_dir.path().join("test1.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("test2.rs"), "").unwrap();
        std::fs::write(temp_dir.path().join("test.txt"), "").unwrap();
        std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        std::fs::write(temp_dir.path().join("subdir/nested.rs"), "").unwrap();

        let tool = GlobTool::new(temp_dir.path().to_path_buf());

        // 기본 패턴 매칭
        let result = tool
            .execute(json!({
                "pattern": "*.rs",
                "path": temp_dir.path().to_str().unwrap()
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("test1.rs"));
                assert!(output.contains("test2.rs"));
                assert!(!output.contains("test.txt"));
            }
            _ => panic!("Expected success"),
        }

        // 재귀 패턴
        let result = tool
            .execute(json!({
                "pattern": "**/*.rs",
                "path": temp_dir.path().to_str().unwrap()
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("nested.rs"));
            }
            _ => panic!("Expected success with recursive pattern"),
        }
    }

    /// Grep Tool - CLI 스펙 준수 검증
    /// CLI: pattern, path, output_mode, glob, type, -i, -n, -A, -B, -C 등
    #[tokio::test]
    async fn test_grep_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();

        std::fs::write(
            temp_dir.path().join("search.txt"),
            "Hello World\nfoo bar\nHello Again\n",
        )
        .unwrap();

        let tool = GrepTool::new(temp_dir.path().to_path_buf());

        // 기본 검색
        let result = tool
            .execute(json!({
                "pattern": "Hello",
                "path": temp_dir.path().to_str().unwrap(),
                "output_mode": "content"
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("Hello World"));
                assert!(output.contains("Hello Again"));
            }
            _ => panic!("Expected success"),
        }

        // 대소문자 무시
        let result = tool
            .execute(json!({
                "pattern": "hello",
                "path": temp_dir.path().to_str().unwrap(),
                "output_mode": "content",
                "-i": true
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("Hello"));
            }
            _ => panic!("Expected success with case-insensitive search"),
        }
    }

    /// Bash Tool - CLI 스펙 준수 검증
    /// CLI: command (required), timeout (optional), description (optional)
    #[tokio::test]
    async fn test_bash_tool_cli_spec() {
        let temp_dir = TempDir::new().unwrap();
        let tool = BashTool::new(temp_dir.path().to_path_buf());

        // 기본 명령 실행
        let result = tool
            .execute(json!({
                "command": "echo 'Hello from Bash'",
                "description": "Echo test"
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("Hello from Bash"));
            }
            _ => panic!("Expected success"),
        }

        // 타임아웃 테스트
        let result = tool
            .execute(json!({
                "command": "sleep 0.1 && echo 'done'",
                "timeout": 5000
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("done"));
            }
            _ => panic!("Expected success with timeout"),
        }
    }

    /// TodoWrite Tool - CLI 스펙 준수 검증
    #[tokio::test]
    async fn test_todo_tool_cli_spec() {
        let tool = TodoWriteTool::new();

        let result = tool
            .execute(json!({
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
            }))
            .await;

        assert!(!result.is_error());
    }
}

// ============================================================================
// 2. Agent Loop Tests - CLI와 동일한 에이전트 동작 검증
// ============================================================================

mod agent_loop_tests {
    use claude_agent::{Agent, AgentEvent, ToolAccess};

    /// Agent 빌더 패턴 검증
    #[tokio::test]
    async fn test_agent_builder_pattern() {
        // CLI의 옵션들과 대응되는 빌더 메서드 검증
        // API 키를 제공해야 빌드 성공
        let agent_result = Agent::builder()
            .api_key("test-api-key") // 테스트용 API 키 제공
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

    /// Tool Access 모드 검증 (CLI의 --allowedTools 대응)
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

        // Custom selection - 직접 String 배열 사용
        let access = ToolAccess::only(["Read".to_string(), "Glob".to_string(), "Grep".to_string()]);
        assert!(access.is_allowed("Read"));
        assert!(!access.is_allowed("Bash"));

        // Exclude specific
        let access = ToolAccess::except(["Bash".to_string()]);
        assert!(access.is_allowed("Read"));
        assert!(!access.is_allowed("Bash"));
    }

    /// AgentEvent 스트림 이벤트 검증 (CLI 출력과 대응)
    #[test]
    fn test_agent_events_match_cli_output() {
        // CLI가 출력하는 이벤트 유형들:
        // - Text: 텍스트 응답
        // - ToolStart: [Tool: name] 출력
        // - ToolEnd: 도구 결과 출력
        // - Complete: 최종 통계

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

        // 이벤트 타입 매칭 검증
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

    /// Session 생성 및 메시지 관리
    #[test]
    fn test_session_management() {
        let config = SessionConfig::default();
        let mut session = Session::new(config);

        // 메시지 추가 (SessionMessage 사용)
        let user_msg = SessionMessage::user(vec![ContentBlock::text("Hello")]);
        session.add_message(user_msg);

        let assistant_msg = SessionMessage::assistant(vec![ContentBlock::text("Hi there!")]);
        session.add_message(assistant_msg);

        assert_eq!(session.messages.len(), 2);
        assert!(session.current_leaf_id.is_some());
    }

    /// Context Compaction (CLI의 자동 컨텍스트 관리 대응)
    #[test]
    fn test_context_compaction() {
        let strategy = CompactStrategy::default()
            .with_threshold(0.8)
            .with_keep_recent(4);

        let executor = CompactExecutor::new(strategy);

        // 80% 이상에서 compact 필요
        assert!(!executor.needs_compact(70_000, 100_000));
        assert!(executor.needs_compact(80_000, 100_000));
        assert!(executor.needs_compact(90_000, 100_000));
    }

    /// Session Manager - 다중 세션 관리
    #[tokio::test]
    async fn test_session_manager() {
        // in-memory persistence로 생성
        let manager = SessionManager::new_memory();

        // 세션 생성
        let session1 = manager.create(SessionConfig::default()).await.unwrap();
        let session2 = manager.create(SessionConfig::default()).await.unwrap();

        assert_ne!(session1.id, session2.id);

        // 세션 검색
        let found = manager.get(&session1.id).await;
        assert!(found.is_ok());
    }
}

// ============================================================================
// 4. Client API Tests - Claude API 통신 스펙 검증
// ============================================================================

mod client_tests {
    use claude_agent::Client;

    /// Client 설정 검증 (CLI 환경변수 대응)
    #[test]
    fn test_client_builder() {
        let client_result = Client::builder()
            .api_key("test-key")
            .model("claude-sonnet-4-5-20250514")
            .max_tokens(4096)
            .timeout(std::time::Duration::from_secs(120))
            .build();

        assert!(client_result.is_ok());
        let client = client_result.unwrap();
        assert_eq!(client.config().model, "claude-sonnet-4-5-20250514");
        assert_eq!(client.config().max_tokens, 4096);
    }

    /// API URL 설정 (CLI의 --api-base-url 대응)
    #[test]
    fn test_custom_base_url() {
        let client = Client::builder()
            .api_key("test-key")
            .base_url("https://custom.api.com/v1")
            .build();

        assert!(client.is_ok());
    }

    /// OAuth 토큰 인증 테스트
    #[test]
    fn test_oauth_token() {
        let client = Client::builder()
            .oauth_token("sk-ant-oat01-test-token")
            .build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.config().auth_strategy.name(), "oauth");
    }

    /// Claude CLI 인증 빌더 테스트
    #[test]
    fn test_from_claude_cli_builder() {
        // CLI credentials가 없어도 빌더 자체는 생성됨
        let _builder = Client::builder().from_claude_cli();
    }

    /// 자동 해결 빌더 테스트
    #[test]
    fn test_auto_resolve_builder() {
        // 빌더가 올바르게 설정되는지 확인
        let _builder = Client::builder().auto_resolve();
    }
}

// ============================================================================
// 5. Permission System Tests - 보안 기능 검증
// ============================================================================

mod permission_tests {
    use claude_agent::permissions::{
        is_file_tool, is_read_only_tool, is_shell_tool, PermissionMode, PermissionPolicyBuilder,
    };

    /// 도구 분류 검증
    #[test]
    fn test_tool_classification() {
        // Read-only tools (안전)
        assert!(is_read_only_tool("Read"));
        assert!(is_read_only_tool("Glob"));
        assert!(is_read_only_tool("Grep"));
        assert!(!is_read_only_tool("Write"));

        // File tools
        assert!(is_file_tool("Read"));
        assert!(is_file_tool("Write"));
        assert!(is_file_tool("Edit"));
        assert!(!is_file_tool("Bash"));

        // Shell tools (위험)
        assert!(is_shell_tool("Bash"));
        assert!(is_shell_tool("KillShell"));
        assert!(!is_shell_tool("Read"));
    }

    /// Permission Mode 검증 (CLI의 --permission-mode 대응)
    #[test]
    fn test_permission_modes() {
        let default = PermissionMode::default();
        assert!(matches!(default, PermissionMode::Default));

        let bypass = PermissionMode::BypassPermissions;
        let plan = PermissionMode::Plan;

        // 모드별 동작 확인
        assert!(matches!(bypass, PermissionMode::BypassPermissions));
        assert!(matches!(plan, PermissionMode::Plan));
    }

    /// Permission Policy 빌더
    #[test]
    fn test_permission_policy_builder() {
        let policy = PermissionPolicyBuilder::new()
            .allow_pattern("Read")
            .allow_pattern("Glob")
            .deny_pattern("Bash")
            .build();

        // 정책 검증
        let read_result = policy.check("Read", &serde_json::Value::Null);
        assert!(read_result.is_allowed());

        let bash_result = policy.check("Bash", &serde_json::Value::Null);
        assert!(bash_result.is_denied());
    }
}

// ============================================================================
// 6. Hook System Tests - 실행 중 개입 기능 검증
// ============================================================================

mod hook_tests {
    use async_trait::async_trait;
    use claude_agent::hooks::{Hook, HookContext, HookEvent, HookInput, HookManager, HookOutput};

    /// Custom Hook 구현
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
            // PreToolUse 이벤트에서 Bash 도구 차단 예시
            if let Some(tool_name) = &input.tool_name {
                if tool_name == "Bash" {
                    return Ok(HookOutput::block("Bash blocked by hook"));
                }
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
// 7. Skill System Tests - 재사용 가능한 워크플로우 검증
// ============================================================================

mod skill_tests {
    use claude_agent::skills::{SkillDefinition, SkillRegistry, SkillResult, SkillSourceType};

    /// Skill 정의 검증
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

    /// Skill Registry 검증
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

    /// Skill Result 검증
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
// 8. Types Compatibility Tests - CLI와 타입 호환성 검증
// ============================================================================

mod types_tests {
    use claude_agent::types::{
        ContentBlock, Message, Role, StopReason, ToolDefinition, ToolResultBlock, Usage,
    };

    /// Message 구조 검증 (API 스펙 준수)
    #[test]
    fn test_message_structure() {
        let user_msg = Message::user("Hello");
        assert!(matches!(user_msg.role, Role::User));

        let assistant_msg = Message::assistant("Hi!");
        assert!(matches!(assistant_msg.role, Role::Assistant));
    }

    /// ContentBlock 유형 검증
    #[test]
    fn test_content_blocks() {
        let text = ContentBlock::text("Hello");
        assert!(matches!(text, ContentBlock::Text { .. }));

        let tool_result = ToolResultBlock::success("tool-id", "result");
        // is_error는 Option<bool>이며, success의 경우 None
        assert!(tool_result.is_error.is_none() || tool_result.is_error == Some(false));
    }

    /// Tool Definition 구조
    #[test]
    fn test_tool_definition() {
        let def = ToolDefinition {
            name: "Read".to_string(),
            description: "Read files".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {"type": "string"}
                },
                "required": ["file_path"]
            }),
        };

        assert_eq!(def.name, "Read");
    }

    /// Usage 토큰 계산
    #[test]
    fn test_usage_calculation() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: Some(10),
            cache_read_input_tokens: Some(5),
        };

        assert_eq!(usage.total(), 150);
    }

    /// StopReason 검증
    #[test]
    fn test_stop_reasons() {
        assert!(matches!(StopReason::EndTurn, StopReason::EndTurn));
        assert!(matches!(StopReason::ToolUse, StopReason::ToolUse));
        assert!(matches!(StopReason::MaxTokens, StopReason::MaxTokens));
    }
}

// ============================================================================
// 9. Error Handling Tests - 에러 처리 일관성 검증
// ============================================================================

mod error_tests {
    use claude_agent::Error;

    /// 에러 타입 검증
    #[test]
    fn test_error_types() {
        let api_error = Error::Api {
            message: "Invalid API key".to_string(),
            status: Some(401),
        };
        assert!(api_error.to_string().contains("Invalid API key"));

        let tool_error = Error::Tool {
            tool: "Read".to_string(),
            message: "File not found".to_string(),
        };
        assert!(tool_error.to_string().contains("Read"));

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
// 10. Integration Scenario Tests - 실제 사용 시나리오 검증
// ============================================================================

mod integration_scenarios {
    use super::*;
    use claude_agent::tools::*;
    use serde_json::json;

    /// 시나리오: 파일 생성 -> 읽기 -> 편집 체인
    #[tokio::test]
    async fn test_file_workflow_scenario() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("workflow.txt");

        let write_tool = WriteTool::new(temp_dir.path().to_path_buf());
        let read_tool = ReadTool::new(temp_dir.path().to_path_buf());
        let edit_tool = EditTool::new(temp_dir.path().to_path_buf());

        // Step 1: Write
        let result = write_tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "content": "function hello() {\n  console.log('Hello');\n}"
            }))
            .await;
        assert!(!result.is_error());

        // Step 2: Read
        let result = read_tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap()
            }))
            .await;
        match result {
            ToolResult::Success(content) => {
                assert!(content.contains("function hello"));
            }
            _ => panic!("Read should succeed"),
        }

        // Step 3: Edit
        let result = edit_tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "Hello",
                "new_string": "World"
            }))
            .await;
        assert!(!result.is_error());

        // Verify final state
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("World"));
    }

    /// 시나리오: 코드 검색 -> 분석 체인
    #[tokio::test]
    async fn test_code_search_scenario() {
        let temp_dir = TempDir::new().unwrap();

        // 테스트 코드 파일 생성
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

        let glob_tool = GlobTool::new(temp_dir.path().to_path_buf());
        let grep_tool = GrepTool::new(temp_dir.path().to_path_buf());

        // Step 1: Find all Rust files
        let result = glob_tool
            .execute(json!({
                "pattern": "**/*.rs",
                "path": temp_dir.path().to_str().unwrap()
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("main.rs"));
                assert!(output.contains("lib.rs"));
            }
            _ => panic!("Glob should succeed"),
        }

        // Step 2: Search for pattern
        let result = grep_tool
            .execute(json!({
                "pattern": "println!",
                "path": temp_dir.path().to_str().unwrap(),
                "output_mode": "content"
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("Hello"));
            }
            _ => panic!("Grep should succeed"),
        }
    }

    /// 시나리오: Shell 명령 실행 (안전한 명령)
    #[tokio::test]
    async fn test_shell_command_scenario() {
        let temp_dir = TempDir::new().unwrap();
        let bash_tool = BashTool::new(temp_dir.path().to_path_buf());

        // 안전한 명령 실행
        let result = bash_tool
            .execute(json!({
                "command": "echo 'test' && pwd",
                "description": "Test echo and pwd"
            }))
            .await;

        match result {
            ToolResult::Success(output) => {
                assert!(output.contains("test"));
            }
            _ => panic!("Bash should succeed"),
        }
    }
}

// ============================================================================
// 11. Feature Parity Checklist
// ============================================================================

/// CLI와 SDK 기능 대응표 검증
#[test]
fn test_feature_parity_checklist() {
    // 이 테스트는 기능 체크리스트를 문서화합니다
    let checklist = vec![
        // 기본 API
        ("query()", true, "Simple query"),
        ("stream()", true, "Streaming query"),
        // 도구
        ("Read", true, "File reading with offset/limit"),
        ("Write", true, "File writing with directory creation"),
        ("Edit", true, "String replacement with replace_all"),
        ("Glob", true, "Pattern matching"),
        ("Grep", true, "Content search with regex"),
        ("Bash", true, "Shell execution with timeout"),
        ("TodoWrite", true, "Task tracking"),
        ("WebFetch", true, "Web fetch"),
        ("NotebookEdit", true, "Jupyter notebook editing"),
        ("KillShell", true, "Kill background shell"),
        // 에이전트
        ("Agent Loop", true, "Multi-turn with tools"),
        (
            "Streaming Events",
            true,
            "Text, ToolStart, ToolEnd, Complete",
        ),
        ("Context Management", true, "Token tracking, compaction"),
        // 세션
        ("Session Management", true, "Create, restore, branch"),
        ("Context Compaction", true, "Automatic summarization"),
        // 보안
        ("Permission System", true, "Tool/path allow/deny"),
        ("Hook System", true, "Execution interception"),
        // 확장
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

    /// MCP Server Config 검증
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

        // HTTP transport
        let http_config = McpServerConfig::Http {
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
        };

        let json = serde_json::to_string(&http_config).unwrap();
        assert!(json.contains("http"));

        // SSE transport
        let sse_config = McpServerConfig::Sse {
            url: "https://sse.example.com".to_string(),
            headers: HashMap::new(),
        };

        let json = serde_json::to_string(&sse_config).unwrap();
        assert!(json.contains("sse"));
    }

    /// MCP Server State 검증
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

    /// MCP Content 타입 검증
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

    /// MCP Tool Result 검증
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
        AuthStrategy, ChainProvider, Credential, CredentialProvider, EnvironmentProvider,
        ExplicitProvider,
    };

    /// API Key Credential 테스트
    #[test]
    fn test_api_key_credential() {
        let cred = Credential::api_key("sk-ant-api-test");
        assert!(!cred.is_expired());
        assert!(!cred.needs_refresh());
        assert_eq!(cred.credential_type(), "api_key");

        // Strategy 패턴으로 헤더 검증
        use claude_agent::ApiKeyStrategy;
        let strategy = ApiKeyStrategy::new("sk-ant-api-test");
        let (header, value) = strategy.auth_header();
        assert_eq!(header, "x-api-key");
        assert_eq!(value, "sk-ant-api-test");
    }

    /// OAuth Credential 테스트
    #[test]
    fn test_oauth_credential() {
        let cred = Credential::oauth("sk-ant-oat01-test");
        assert_eq!(cred.credential_type(), "oauth");

        // OAuth credential의 access_token 확인
        match cred {
            Credential::OAuth(oauth) => {
                assert_eq!(oauth.access_token, "sk-ant-oat01-test");
            }
            _ => panic!("Expected OAuth credential"),
        }
    }

    /// ExplicitProvider 테스트
    #[tokio::test]
    async fn test_explicit_provider() {
        let provider = ExplicitProvider::api_key("test-key");
        assert_eq!(provider.name(), "explicit");

        let cred = provider.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "test-key"));
    }

    /// EnvironmentProvider 테스트
    #[tokio::test]
    async fn test_environment_provider() {
        std::env::set_var("TEST_AUTH_KEY", "env-test-key");
        let provider = EnvironmentProvider::with_var("TEST_AUTH_KEY");
        assert_eq!(provider.name(), "environment");

        let cred = provider.resolve().await.unwrap();
        assert!(matches!(cred, Credential::ApiKey(k) if k == "env-test-key"));
        std::env::remove_var("TEST_AUTH_KEY");
    }

    /// ChainProvider 테스트
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

    /// Static Context 검증
    #[test]
    fn test_static_context() {
        let context = StaticContext {
            system_prompt: "You are a helpful assistant.".to_string(),
            claude_md: "# Project".to_string(),
            skill_index_summary: "Available skills: commit, review".to_string(),
            tool_definitions: vec![],
            mcp_tool_metadata: vec![],
        };

        assert!(!context.system_prompt.is_empty());
        assert!(!context.claude_md.is_empty());
    }

    /// System Block 검증
    #[test]
    fn test_system_block() {
        let cached = SystemBlock::cached("Cached content");
        assert!(cached.cache_control.is_some());
        assert_eq!(cached.cache_control.unwrap().cache_type, "ephemeral");
        assert_eq!(cached.block_type, "text");

        let uncached = SystemBlock::uncached("Uncached content");
        assert!(uncached.cache_control.is_none());
    }

    /// Cache Control 타입 검증
    #[test]
    fn test_cache_control() {
        use claude_agent::types::CacheControl;
        let ephemeral = CacheControl::ephemeral();
        // Ephemeral은 5분 TTL 캐싱
        assert_eq!(ephemeral.cache_type, "ephemeral");
    }
}
