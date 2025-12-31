//! Agent executor - the main agent loop.

use std::collections::VecDeque;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::Mutex;

use super::{AgentOptions, ConversationContext};
use crate::extension::ExtensionRegistry;
use crate::hooks::{HookContext, HookEvent, HookInput, HookManager};
use crate::types::{ContentBlock, Message, StopReason, ToolResultBlock, ToolUseBlock, Usage};
use crate::{Client, Result, ToolRegistry};

/// Events emitted during agent execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Text content from Claude
    Text(String),
    /// Tool execution started
    ToolStart {
        /// Tool use ID
        id: String,
        /// Tool name
        name: String,
        /// Tool input
        input: serde_json::Value,
    },
    /// Tool execution completed
    ToolEnd {
        /// Tool use ID
        id: String,
        /// Tool output
        output: String,
        /// Whether it was an error
        is_error: bool,
    },
    /// Thinking/reasoning (if extended thinking enabled)
    Thinking(String),
    /// Agent execution completed
    Complete(AgentResult),
}

/// Result of agent execution
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// Final text response
    pub text: String,
    /// Total token usage
    pub usage: Usage,
    /// Number of tool calls made
    pub tool_calls: usize,
    /// Number of iterations
    pub iterations: usize,
    /// Stop reason
    pub stop_reason: StopReason,
}

impl AgentResult {
    /// Get the final text response
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get total tokens used
    pub fn total_tokens(&self) -> u32 {
        self.usage.total()
    }
}

/// The main agent executor.
pub struct Agent {
    client: Client,
    options: AgentOptions,
    tools: ToolRegistry,
    hooks: HookManager,
    extensions: ExtensionRegistry,
    session_id: String,
}

impl Agent {
    /// Creates a new agent with default tools and empty hooks.
    pub fn new(client: Client, options: AgentOptions) -> Self {
        let tools = ToolRegistry::default_tools(&options.tool_access, options.working_dir.clone());
        Self {
            client,
            options,
            tools,
            hooks: HookManager::new(),
            extensions: ExtensionRegistry::new(),
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Creates a new agent with custom skill executor.
    pub fn with_skills(
        client: Client,
        options: AgentOptions,
        skill_executor: crate::skills::SkillExecutor,
    ) -> Self {
        let tools = ToolRegistry::with_skills(
            &options.tool_access,
            options.working_dir.clone(),
            skill_executor,
        );
        Self {
            client,
            options,
            tools,
            hooks: HookManager::new(),
            extensions: ExtensionRegistry::new(),
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Creates a new agent with all components.
    pub fn with_components(
        client: Client,
        options: AgentOptions,
        tools: ToolRegistry,
        hooks: HookManager,
        extensions: ExtensionRegistry,
    ) -> Self {
        Self {
            client,
            options,
            tools,
            hooks,
            extensions,
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Creates an agent builder.
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    /// Returns the hook manager.
    pub fn hooks(&self) -> &HookManager {
        &self.hooks
    }

    /// Returns the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Creates a hook context for this agent.
    fn hook_context(&self) -> HookContext {
        HookContext::new(&self.session_id)
            .with_cwd(self.options.working_dir.clone().unwrap_or_default())
    }

    /// Execute a prompt (non-streaming)
    pub async fn execute(&self, prompt: &str) -> Result<AgentResult> {
        let mut context = ConversationContext::new();
        context.push(Message::user(prompt));

        let mut iterations = 0;
        let mut tool_calls = 0;
        let mut final_text = String::new();
        let mut final_stop_reason = StopReason::EndTurn;

        loop {
            iterations += 1;
            if iterations > self.options.max_iterations {
                break;
            }

            // Build and send request
            let request = self.build_request(&context);
            let response = crate::client::MessagesClient::new(&self.client)
                .create(request)
                .await?;

            context.update_usage(response.usage);
            final_text = response.text();
            final_stop_reason = response.stop_reason.unwrap_or(StopReason::EndTurn);

            // Add assistant message to context
            context.push(Message {
                role: crate::types::Role::Assistant,
                content: response.content.clone(),
            });

            // Check if we need to execute tools
            if !response.wants_tool_use() {
                break;
            }

            // Execute tools with hooks
            let tool_uses = response.tool_uses();
            let mut results = Vec::new();
            let hook_ctx = self.hook_context();

            for tool_use in tool_uses {
                // Pre-tool hook
                let pre_input =
                    HookInput::pre_tool_use(&self.session_id, &tool_use.name, tool_use.input.clone());
                let pre_output = self
                    .hooks
                    .execute(HookEvent::PreToolUse, pre_input, &hook_ctx)
                    .await?;

                if !pre_output.continue_execution {
                    results.push(ToolResultBlock::error(
                        &tool_use.id,
                        pre_output.stop_reason.unwrap_or_else(|| "Blocked by hook".into()),
                    ));
                    continue;
                }

                // Use updated input if provided
                let input = pre_output.updated_input.unwrap_or(tool_use.input.clone());

                tool_calls += 1;
                let result = self.tools.execute(&tool_use.name, input).await;

                // Post-tool hook
                let post_input =
                    HookInput::post_tool_use(&self.session_id, &tool_use.name, result.clone());
                let _ = self
                    .hooks
                    .execute(HookEvent::PostToolUse, post_input, &hook_ctx)
                    .await;

                results.push(ToolResultBlock::from_output(&tool_use.id, result));
            }

            // Add tool results to context
            context.push(Message::tool_results(results));

            // Check for compaction
            if self.options.auto_compact
                && context.should_compact(200_000, self.options.compact_threshold)
            {
                context.compact(&self.client).await?;
            }
        }

        Ok(AgentResult {
            text: final_text,
            usage: *context.total_usage(),
            tool_calls,
            iterations,
            stop_reason: final_stop_reason,
        })
    }

    /// Execute a prompt with streaming
    pub async fn execute_stream(
        &self,
        prompt: &str,
    ) -> Result<impl Stream<Item = Result<AgentEvent>>> {
        use futures::stream;

        let context = Arc::new(Mutex::new(ConversationContext::new()));
        {
            let mut ctx = context.lock().await;
            ctx.push(Message::user(prompt));
        }

        let client = self.client.clone();
        let options = self.options.clone();
        let tools = Arc::new(ToolRegistry::default_tools(
            &self.options.tool_access,
            self.options.working_dir.clone(),
        ));
        let system_prompt = self.build_system_prompt();

        // Create an async stream that yields AgentEvents
        let event_stream = stream::unfold(
            StreamState::new(context, client, options, tools, system_prompt),
            |mut state| async move { state.next_event().await.map(|event| (event, state)) },
        );

        Ok(event_stream)
    }

    /// Build a request from context
    fn build_request(
        &self,
        context: &ConversationContext,
    ) -> crate::client::messages::CreateMessageRequest {
        let mut request = crate::client::messages::CreateMessageRequest::new(
            &self.options.model,
            context.messages().to_vec(),
        )
        .with_max_tokens(self.options.max_tokens);

        // Add system prompt
        let system_prompt = self.build_system_prompt();
        request = request.with_system(system_prompt);

        // Add tool definitions
        let tool_defs = self.tools.definitions();
        if !tool_defs.is_empty() {
            request = request.with_tools(tool_defs);
        }

        request
    }

    /// Build the system prompt
    fn build_system_prompt(&self) -> String {
        let mut prompt = crate::prompts::system::MAIN_PROMPT.to_string();

        if let Some(custom) = &self.options.system_prompt {
            prompt.push_str("\n\n");
            prompt.push_str(custom);
        }

        prompt
    }
}

impl ToolResultBlock {
    /// Create from tool output
    fn from_output(tool_use_id: &str, output: crate::tools::ToolResult) -> Self {
        match output {
            crate::tools::ToolResult::Success(content) => Self::success(tool_use_id, content),
            crate::tools::ToolResult::Error(message) => Self::error(tool_use_id, message),
            crate::tools::ToolResult::Empty => Self::empty(tool_use_id),
        }
    }
}

/// Internal state machine for streaming agent execution
struct StreamState {
    context: Arc<Mutex<ConversationContext>>,
    client: Client,
    options: AgentOptions,
    tools: Arc<ToolRegistry>,
    system_prompt: String,
    iterations: usize,
    tool_calls: usize,
    pending_events: VecDeque<Result<AgentEvent>>,
    pending_tool_results: Vec<ToolResultBlock>,
    pending_tool_uses: Vec<ToolUseBlock>,
    final_text: String,
    done: bool,
}

impl StreamState {
    fn new(
        context: Arc<Mutex<ConversationContext>>,
        client: Client,
        options: AgentOptions,
        tools: Arc<ToolRegistry>,
        system_prompt: String,
    ) -> Self {
        Self {
            context,
            client,
            options,
            tools,
            system_prompt,
            iterations: 0,
            tool_calls: 0,
            pending_events: VecDeque::new(),
            pending_tool_results: Vec::new(),
            pending_tool_uses: Vec::new(),
            final_text: String::new(),
            done: false,
        }
    }

    async fn next_event(&mut self) -> Option<Result<AgentEvent>> {
        // Return any pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Some(event);
        }

        if self.done {
            return None;
        }

        // Execute any pending tool calls
        if !self.pending_tool_uses.is_empty() {
            let tool_use = self.pending_tool_uses.remove(0);
            self.tool_calls += 1;

            // Emit ToolStart event
            self.pending_events.push_back(Ok(AgentEvent::ToolStart {
                id: tool_use.id.clone(),
                name: tool_use.name.clone(),
                input: tool_use.input.clone(),
            }));

            // Execute the tool
            let result = self
                .tools
                .execute(&tool_use.name, tool_use.input.clone())
                .await;
            let (output, is_error) = match &result {
                crate::tools::ToolResult::Success(s) => (s.clone(), false),
                crate::tools::ToolResult::Error(s) => (s.clone(), true),
                crate::tools::ToolResult::Empty => (String::new(), false),
            };

            // Queue ToolEnd event
            self.pending_events.push_back(Ok(AgentEvent::ToolEnd {
                id: tool_use.id.clone(),
                output: output.clone(),
                is_error,
            }));

            // Store tool result
            self.pending_tool_results
                .push(ToolResultBlock::from_output(&tool_use.id, result));

            // If no more pending tool uses, add tool results to context and continue
            if self.pending_tool_uses.is_empty() && !self.pending_tool_results.is_empty() {
                let results = std::mem::take(&mut self.pending_tool_results);
                let mut ctx = self.context.lock().await;
                ctx.push(Message::tool_results(results));
            }

            return self.pending_events.pop_front();
        }

        // Check iteration limit
        self.iterations += 1;
        if self.iterations > self.options.max_iterations {
            self.done = true;
            let ctx = self.context.lock().await;
            return Some(Ok(AgentEvent::Complete(AgentResult {
                text: self.final_text.clone(),
                usage: *ctx.total_usage(),
                tool_calls: self.tool_calls,
                iterations: self.iterations - 1,
                stop_reason: StopReason::MaxTokens,
            })));
        }

        // Build and send request
        let request = {
            let ctx = self.context.lock().await;
            let mut req = crate::client::messages::CreateMessageRequest::new(
                &self.options.model,
                ctx.messages().to_vec(),
            )
            .with_max_tokens(self.options.max_tokens)
            .with_system(self.system_prompt.clone());

            let tool_defs = self.tools.definitions();
            if !tool_defs.is_empty() {
                req = req.with_tools(tool_defs);
            }
            req
        };

        // Send request
        let response = match crate::client::MessagesClient::new(&self.client)
            .create(request)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                self.done = true;
                return Some(Err(e));
            }
        };

        // Update context with response
        {
            let mut ctx = self.context.lock().await;
            ctx.update_usage(response.usage);
        }

        // Process response content
        let mut text_content = String::new();
        let mut tool_uses = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    text_content.push_str(text);
                    self.pending_events
                        .push_back(Ok(AgentEvent::Text(text.clone())));
                }
                ContentBlock::ToolUse(tool_use) => {
                    tool_uses.push(tool_use.clone());
                }
                _ => {}
            }
        }

        self.final_text = text_content;

        // Add assistant message to context
        {
            let mut ctx = self.context.lock().await;
            ctx.push(Message {
                role: crate::types::Role::Assistant,
                content: response.content.clone(),
            });
        }

        // Check if we need to execute tools
        if response.wants_tool_use() && !tool_uses.is_empty() {
            self.pending_tool_uses = tool_uses;
        } else {
            // Done - no more tool calls
            self.done = true;
            let ctx = self.context.lock().await;
            self.pending_events
                .push_back(Ok(AgentEvent::Complete(AgentResult {
                    text: self.final_text.clone(),
                    usage: *ctx.total_usage(),
                    tool_calls: self.tool_calls,
                    iterations: self.iterations,
                    stop_reason: response.stop_reason.unwrap_or(StopReason::EndTurn),
                })));
        }

        self.pending_events.pop_front()
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        self.extensions.cleanup_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_result() {
        let result = AgentResult {
            text: "Hello".to_string(),
            usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
            tool_calls: 2,
            iterations: 3,
            stop_reason: StopReason::EndTurn,
        };

        assert_eq!(result.text(), "Hello");
        assert_eq!(result.total_tokens(), 150);
    }
}
