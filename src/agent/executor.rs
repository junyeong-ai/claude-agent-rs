//! Agent executor - the main agent loop.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use futures::Stream;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

use super::{AgentMetrics, AgentOptions, AgentState, ConversationContext};
use crate::extension::ExtensionRegistry;
use crate::hooks::{HookContext, HookEvent, HookInput, HookManager};
use crate::types::CompactResult;
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

/// Result of agent execution.
#[derive(Debug, Clone)]
pub struct AgentResult {
    /// Final text response.
    pub text: String,
    /// Total token usage.
    pub usage: Usage,
    /// Number of tool calls made.
    pub tool_calls: usize,
    /// Number of iterations.
    pub iterations: usize,
    /// Stop reason.
    pub stop_reason: StopReason,
    /// Final agent state.
    pub state: AgentState,
    /// Detailed execution metrics.
    pub metrics: AgentMetrics,
}

impl AgentResult {
    /// Get the final text response.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get total tokens used.
    #[must_use]
    pub fn total_tokens(&self) -> u32 {
        self.usage.total()
    }

    /// Get the detailed metrics.
    #[must_use]
    pub fn metrics(&self) -> &AgentMetrics {
        &self.metrics
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
    #[must_use]
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    /// Returns the hook manager.
    #[must_use]
    pub fn hooks(&self) -> &HookManager {
        &self.hooks
    }

    /// Returns the session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Creates a hook context for this agent.
    fn hook_context(&self) -> HookContext {
        HookContext::new(&self.session_id)
            .with_cwd(self.options.working_dir.clone().unwrap_or_default())
    }

    /// Execute a prompt (non-streaming) with timeout enforcement.
    pub async fn execute(&self, prompt: &str) -> Result<AgentResult> {
        let timeout = self
            .options
            .timeout
            .unwrap_or(std::time::Duration::from_secs(600));

        tokio::time::timeout(timeout, self.execute_inner(prompt))
            .await
            .map_err(|_| crate::Error::Timeout(timeout))?
    }

    /// Internal execution logic without timeout wrapper.
    #[instrument(skip(self, prompt), fields(session_id = %self.session_id))]
    async fn execute_inner(&self, prompt: &str) -> Result<AgentResult> {
        let execution_start = Instant::now();
        let mut context = ConversationContext::new();
        context.push(Message::user(prompt));

        let mut metrics = AgentMetrics::default();
        let mut final_text = String::new();
        let mut final_stop_reason = StopReason::EndTurn;

        info!(prompt_len = prompt.len(), "Starting agent execution");

        loop {
            metrics.iterations += 1;
            if metrics.iterations > self.options.max_iterations {
                warn!(max = self.options.max_iterations, "Max iterations reached");
                break;
            }

            debug!(iteration = metrics.iterations, "Starting iteration");

            let api_start = Instant::now();
            let request = self.build_request(&context);
            let response = crate::client::MessagesClient::new(&self.client)
                .create(request)
                .await?;
            metrics.record_api_call();
            debug!(
                api_time_ms = api_start.elapsed().as_millis(),
                "API call completed"
            );

            context.update_usage(response.usage);
            metrics.add_usage(response.usage.input_tokens, response.usage.output_tokens);
            final_text = response.text();
            final_stop_reason = response.stop_reason.unwrap_or(StopReason::EndTurn);

            context.push(Message {
                role: crate::types::Role::Assistant,
                content: response.content.clone(),
            });

            if !response.wants_tool_use() {
                debug!("No tool use requested, ending loop");
                break;
            }

            let tool_uses = response.tool_uses();
            let hook_ctx = self.hook_context();

            let mut prepared: Vec<(String, String, serde_json::Value)> = Vec::new();
            let mut blocked: Vec<ToolResultBlock> = Vec::new();

            for tool_use in &tool_uses {
                let pre_input = HookInput::pre_tool_use(
                    &self.session_id,
                    &tool_use.name,
                    tool_use.input.clone(),
                );
                let pre_output = self
                    .hooks
                    .execute(HookEvent::PreToolUse, pre_input, &hook_ctx)
                    .await?;

                if !pre_output.continue_execution {
                    debug!(tool = %tool_use.name, "Tool blocked by hook");
                    blocked.push(ToolResultBlock::error(
                        &tool_use.id,
                        pre_output
                            .stop_reason
                            .unwrap_or_else(|| "Blocked by hook".into()),
                    ));
                } else {
                    let input = pre_output.updated_input.unwrap_or(tool_use.input.clone());
                    prepared.push((tool_use.id.clone(), tool_use.name.clone(), input));
                }
            }

            let tool_futures = prepared.iter().map(|(id, name, input)| {
                let id = id.clone();
                let name = name.clone();
                let input = input.clone();
                let tools = &self.tools;
                async move {
                    let start = Instant::now();
                    let result = tools.execute(&name, input).await;
                    let duration_ms = start.elapsed().as_millis() as u64;
                    (id, name, result, duration_ms)
                }
            });

            let parallel_results: Vec<_> = futures::future::join_all(tool_futures).await;

            let mut results = blocked;
            for (id, name, result, duration_ms) in parallel_results {
                let is_error = result.is_error();
                debug!(tool = %name, duration_ms, is_error, "Tool execution completed");
                metrics.record_tool(&name, duration_ms, is_error);

                let post_input = HookInput::post_tool_use(&self.session_id, &name, result.clone());
                let _ = self
                    .hooks
                    .execute(HookEvent::PostToolUse, post_input, &hook_ctx)
                    .await;
                results.push(ToolResultBlock::from_output(&id, result));
            }

            context.push(Message::tool_results(results));

            if self.options.auto_compact
                && context.should_compact(200_000, self.options.compact_threshold)
            {
                debug!("Compacting context");
                if let Ok(CompactResult::Compacted { saved_tokens, .. }) =
                    context.compact(&self.client).await
                {
                    info!(saved_tokens, "Context compacted");
                    metrics.record_compaction();
                }
            }
        }

        metrics.execution_time_ms = execution_start.elapsed().as_millis() as u64;

        info!(
            iterations = metrics.iterations,
            tool_calls = metrics.tool_calls,
            api_calls = metrics.api_calls,
            total_tokens = metrics.total_tokens(),
            execution_time_ms = metrics.execution_time_ms,
            "Agent execution completed"
        );

        Ok(AgentResult {
            text: final_text,
            usage: *context.total_usage(),
            tool_calls: metrics.tool_calls,
            iterations: metrics.iterations,
            stop_reason: final_stop_reason,
            state: AgentState::Completed,
            metrics,
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
        let tools = Arc::new(self.tools.clone());
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

/// Internal state machine for streaming agent execution.
struct StreamState {
    context: Arc<Mutex<ConversationContext>>,
    client: Client,
    options: AgentOptions,
    tools: Arc<ToolRegistry>,
    system_prompt: String,
    metrics: AgentMetrics,
    start_time: Instant,
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
            metrics: AgentMetrics::default(),
            start_time: Instant::now(),
            pending_events: VecDeque::new(),
            pending_tool_results: Vec::new(),
            pending_tool_uses: Vec::new(),
            final_text: String::new(),
            done: false,
        }
    }

    async fn next_event(&mut self) -> Option<Result<AgentEvent>> {
        if let Some(event) = self.pending_events.pop_front() {
            return Some(event);
        }

        if self.done {
            return None;
        }

        if !self.pending_tool_uses.is_empty() {
            let tool_use = self.pending_tool_uses.remove(0);

            self.pending_events.push_back(Ok(AgentEvent::ToolStart {
                id: tool_use.id.clone(),
                name: tool_use.name.clone(),
                input: tool_use.input.clone(),
            }));

            let start = Instant::now();
            let result = self
                .tools
                .execute(&tool_use.name, tool_use.input.clone())
                .await;
            let duration_ms = start.elapsed().as_millis() as u64;

            let (output, is_error) = match &result {
                crate::tools::ToolResult::Success(s) => (s.clone(), false),
                crate::tools::ToolResult::Error(s) => (s.clone(), true),
                crate::tools::ToolResult::Empty => (String::new(), false),
            };

            self.metrics
                .record_tool(&tool_use.name, duration_ms, is_error);

            self.pending_events.push_back(Ok(AgentEvent::ToolEnd {
                id: tool_use.id.clone(),
                output: output.clone(),
                is_error,
            }));

            self.pending_tool_results
                .push(ToolResultBlock::from_output(&tool_use.id, result));

            if self.pending_tool_uses.is_empty() && !self.pending_tool_results.is_empty() {
                let results = std::mem::take(&mut self.pending_tool_results);
                let mut ctx = self.context.lock().await;
                ctx.push(Message::tool_results(results));
            }

            return self.pending_events.pop_front();
        }

        self.metrics.iterations += 1;
        if self.metrics.iterations > self.options.max_iterations {
            self.done = true;
            self.metrics.execution_time_ms = self.start_time.elapsed().as_millis() as u64;
            let ctx = self.context.lock().await;
            return Some(Ok(AgentEvent::Complete(AgentResult {
                text: self.final_text.clone(),
                usage: *ctx.total_usage(),
                tool_calls: self.metrics.tool_calls,
                iterations: self.metrics.iterations - 1,
                stop_reason: StopReason::MaxTokens,
                state: AgentState::Completed,
                metrics: self.metrics.clone(),
            })));
        }

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

        self.metrics.record_api_call();
        self.metrics
            .add_usage(response.usage.input_tokens, response.usage.output_tokens);

        {
            let mut ctx = self.context.lock().await;
            ctx.update_usage(response.usage);
        }

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

        {
            let mut ctx = self.context.lock().await;
            ctx.push(Message {
                role: crate::types::Role::Assistant,
                content: response.content.clone(),
            });
        }

        if response.wants_tool_use() && !tool_uses.is_empty() {
            self.pending_tool_uses = tool_uses;
        } else {
            self.done = true;
            self.metrics.execution_time_ms = self.start_time.elapsed().as_millis() as u64;
            let ctx = self.context.lock().await;
            self.pending_events
                .push_back(Ok(AgentEvent::Complete(AgentResult {
                    text: self.final_text.clone(),
                    usage: *ctx.total_usage(),
                    tool_calls: self.metrics.tool_calls,
                    iterations: self.metrics.iterations,
                    stop_reason: response.stop_reason.unwrap_or(StopReason::EndTurn),
                    state: AgentState::Completed,
                    metrics: self.metrics.clone(),
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
        let metrics = AgentMetrics {
            iterations: 3,
            tool_calls: 2,
            ..Default::default()
        };

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
            state: AgentState::Completed,
            metrics,
        };

        assert_eq!(result.text(), "Hello");
        assert_eq!(result.total_tokens(), 150);
        assert!(result.state.is_terminal());
        assert_eq!(result.metrics().iterations, 3);
    }
}
