//! Agent streaming execution with session-based context management.

use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use futures::{Stream, StreamExt, stream};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::common::BudgetContext;
use super::events::{AgentEvent, AgentResult};
use super::execution::extract_file_path;
use super::executor::Agent;
use super::request::RequestBuilder;
use super::state_formatter::collect_compaction_state;
use super::{AgentConfig, AgentMetrics, AgentState};
use crate::budget::{BudgetTracker, TenantBudget};
use crate::client::{RecoverableStream, StreamItem};
use crate::context::PromptOrchestrator;
use crate::hooks::{HookContext, HookEvent, HookInput, HookManager};
use crate::session::ToolState;
use crate::types::{
    CompactResult, ContentBlock, PermissionDenial, StopReason, StreamEvent, ToolResultBlock,
    ToolUseBlock, Usage, context_window,
};
use crate::{Client, ToolRegistry};

type BoxedByteStream =
    Pin<Box<dyn Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> + Send>>;

impl Agent {
    pub async fn execute_stream(
        &self,
        prompt: &str,
    ) -> crate::Result<impl Stream<Item = crate::Result<AgentEvent>> + Send> {
        let timeout = self
            .config
            .execution
            .timeout
            .unwrap_or(std::time::Duration::from_secs(600));

        if self.state.is_executing() {
            self.state
                .enqueue(prompt)
                .await
                .map_err(|e| crate::Error::Session(format!("Queue full: {}", e)))?;
        } else {
            self.state
                .with_session_mut(|session| {
                    session.add_user_message(prompt);
                })
                .await;
        }

        let state = StreamState::new(
            StreamStateConfig {
                tool_state: self.state.clone(),
                client: Arc::clone(&self.client),
                config: Arc::clone(&self.config),
                tools: Arc::clone(&self.tools),
                hooks: Arc::clone(&self.hooks),
                hook_context: self.hook_context(),
                request_builder: RequestBuilder::new(&self.config, Arc::clone(&self.tools)),
                orchestrator: self.orchestrator.clone(),
                session_id: Arc::clone(&self.session_id),
                budget_tracker: Arc::clone(&self.budget_tracker),
                tenant_budget: self.tenant_budget.clone(),
            },
            timeout,
        );

        Ok(stream::unfold(state, |mut state| async move {
            state.next_event().await.map(|event| (event, state))
        }))
    }
}

struct StreamStateConfig {
    tool_state: ToolState,
    client: Arc<Client>,
    config: Arc<AgentConfig>,
    tools: Arc<ToolRegistry>,
    hooks: Arc<HookManager>,
    hook_context: HookContext,
    request_builder: RequestBuilder,
    orchestrator: Option<Arc<RwLock<PromptOrchestrator>>>,
    session_id: Arc<str>,
    budget_tracker: Arc<BudgetTracker>,
    tenant_budget: Option<Arc<TenantBudget>>,
}

enum StreamPollResult {
    Event(crate::Result<AgentEvent>),
    Continue,
    StreamEnded,
}

enum Phase {
    StartRequest,
    Streaming(Box<StreamingPhase>),
    StreamEnded { accumulated_usage: Usage },
    ProcessingTools { tool_index: usize },
    Done,
}

struct StreamingPhase {
    stream: RecoverableStream<BoxedByteStream>,
    accumulated_usage: Usage,
}

struct StreamState {
    cfg: StreamStateConfig,
    timeout: std::time::Duration,
    chunk_timeout: std::time::Duration,
    dynamic_rules: String,
    metrics: AgentMetrics,
    start_time: Instant,
    last_chunk_time: Instant,
    pending_tool_results: Vec<ToolResultBlock>,
    pending_tool_uses: Vec<ToolUseBlock>,
    final_text: String,
    total_usage: Usage,
    phase: Phase,
}

impl StreamState {
    fn new(cfg: StreamStateConfig, timeout: std::time::Duration) -> Self {
        let chunk_timeout = cfg.config.execution.chunk_timeout;
        let now = Instant::now();
        Self {
            cfg,
            timeout,
            chunk_timeout,
            dynamic_rules: String::new(),
            metrics: AgentMetrics::default(),
            start_time: now,
            last_chunk_time: now,
            pending_tool_results: Vec::new(),
            pending_tool_uses: Vec::new(),
            final_text: String::new(),
            total_usage: Usage::default(),
            phase: Phase::StartRequest,
        }
    }

    async fn next_event(&mut self) -> Option<crate::Result<AgentEvent>> {
        loop {
            if matches!(self.phase, Phase::Done) {
                return None;
            }

            if self.start_time.elapsed() > self.timeout {
                self.phase = Phase::Done;
                return Some(Err(crate::Error::Timeout(self.timeout)));
            }

            if let Some(event) = self.check_budget_exceeded() {
                return Some(event);
            }

            match std::mem::replace(&mut self.phase, Phase::Done) {
                Phase::StartRequest => {
                    if let Some(result) = self.do_start_request().await {
                        return Some(result);
                    }
                }
                Phase::Streaming(mut streaming) => {
                    match self
                        .do_poll_stream(&mut streaming.stream, &mut streaming.accumulated_usage)
                        .await
                    {
                        StreamPollResult::Event(event) => {
                            self.phase = Phase::Streaming(streaming);
                            return Some(event);
                        }
                        StreamPollResult::Continue => {
                            self.phase = Phase::Streaming(streaming);
                        }
                        StreamPollResult::StreamEnded => {
                            self.phase = Phase::StreamEnded {
                                accumulated_usage: streaming.accumulated_usage,
                            };
                        }
                    }
                }
                Phase::StreamEnded { accumulated_usage } => {
                    if let Some(event) = self.do_handle_stream_end(accumulated_usage).await {
                        return Some(event);
                    }
                }
                Phase::ProcessingTools { tool_index } => {
                    if let Some(result) = self.do_process_tool(tool_index).await {
                        return Some(result);
                    }
                }
                Phase::Done => return None,
            }
        }
    }

    fn check_budget_exceeded(&mut self) -> Option<crate::Result<AgentEvent>> {
        let result = BudgetContext {
            tracker: &self.cfg.budget_tracker,
            tenant: self.cfg.tenant_budget.as_deref(),
            config: &self.cfg.config.budget,
        }
        .check();

        if let Err(e) = result {
            self.phase = Phase::Done;
            return Some(Err(e));
        }

        None
    }

    async fn do_start_request(&mut self) -> Option<crate::Result<AgentEvent>> {
        self.metrics.iterations += 1;
        if self.metrics.iterations > self.cfg.config.execution.max_iterations {
            self.phase = Phase::Done;
            self.metrics.execution_time_ms = self.start_time.elapsed().as_millis() as u64;

            let messages = self
                .cfg
                .tool_state
                .with_session(|session| session.to_api_messages())
                .await;

            return Some(Ok(AgentEvent::Complete(Box::new(AgentResult {
                text: self.final_text.clone(),
                usage: self.total_usage,
                tool_calls: self.metrics.tool_calls,
                iterations: self.metrics.iterations - 1,
                stop_reason: StopReason::MaxTokens,
                state: AgentState::Completed,
                metrics: self.metrics.clone(),
                session_id: self.cfg.session_id.to_string(),
                structured_output: None,
                messages,
                uuid: uuid::Uuid::new_v4().to_string(),
            }))));
        }

        let messages = self
            .cfg
            .tool_state
            .with_session(|session| {
                session.to_api_messages_with_cache(self.cfg.config.cache.message_ttl_option())
            })
            .await;

        let stream_request = self
            .cfg
            .request_builder
            .build(messages, &self.dynamic_rules)
            .with_stream();

        let response = match self
            .cfg
            .client
            .send_stream_with_auth_retry(stream_request)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                self.phase = Phase::Done;
                return Some(Err(e));
            }
        };

        self.metrics.record_api_call();

        let boxed_stream: BoxedByteStream = Box::pin(response.bytes_stream());
        self.phase = Phase::Streaming(Box::new(StreamingPhase {
            stream: RecoverableStream::new(boxed_stream),
            accumulated_usage: Usage::default(),
        }));

        None
    }

    async fn do_poll_stream(
        &mut self,
        stream: &mut RecoverableStream<BoxedByteStream>,
        accumulated_usage: &mut Usage,
    ) -> StreamPollResult {
        let chunk_result = tokio::time::timeout(self.chunk_timeout, stream.next()).await;

        match chunk_result {
            Ok(Some(Ok(item))) => {
                self.last_chunk_time = Instant::now();
                self.handle_stream_item(item, accumulated_usage)
            }
            Ok(Some(Err(e))) => {
                self.phase = Phase::Done;
                StreamPollResult::Event(Err(e))
            }
            Ok(None) => StreamPollResult::StreamEnded,
            Err(_) => {
                self.phase = Phase::Done;
                StreamPollResult::Event(Err(crate::Error::Stream(format!(
                    "Chunk timeout after {:?} (no data received)",
                    self.chunk_timeout
                ))))
            }
        }
    }

    fn handle_stream_item(
        &mut self,
        item: StreamItem,
        accumulated_usage: &mut Usage,
    ) -> StreamPollResult {
        match item {
            StreamItem::Text(text) => {
                self.final_text.push_str(&text);
                StreamPollResult::Event(Ok(AgentEvent::Text(text)))
            }
            StreamItem::Thinking(thinking) => {
                StreamPollResult::Event(Ok(AgentEvent::Thinking(thinking)))
            }
            StreamItem::Citation(_) => StreamPollResult::Continue,
            StreamItem::ToolUseComplete(tool_use) => {
                self.pending_tool_uses.push(tool_use);
                StreamPollResult::Continue
            }
            StreamItem::Event(event) => self.handle_stream_event(event, accumulated_usage),
        }
    }

    fn handle_stream_event(
        &mut self,
        event: StreamEvent,
        accumulated_usage: &mut Usage,
    ) -> StreamPollResult {
        match event {
            StreamEvent::MessageStart { message } => {
                accumulated_usage.input_tokens = message.usage.input_tokens;
                accumulated_usage.output_tokens = message.usage.output_tokens;
                accumulated_usage.cache_creation_input_tokens =
                    message.usage.cache_creation_input_tokens;
                accumulated_usage.cache_read_input_tokens = message.usage.cache_read_input_tokens;
                StreamPollResult::Continue
            }
            StreamEvent::ContentBlockStart { .. } => StreamPollResult::Continue,
            StreamEvent::ContentBlockDelta { .. } => StreamPollResult::Continue,
            StreamEvent::ContentBlockStop { .. } => StreamPollResult::Continue,
            StreamEvent::MessageDelta { usage, .. } => {
                accumulated_usage.output_tokens = usage.output_tokens;
                StreamPollResult::Continue
            }
            StreamEvent::MessageStop => StreamPollResult::StreamEnded,
            StreamEvent::Ping => StreamPollResult::Continue,
            StreamEvent::Error { error } => {
                self.phase = Phase::Done;
                StreamPollResult::Event(Err(crate::Error::Stream(error.message)))
            }
        }
    }

    async fn do_handle_stream_end(
        &mut self,
        accumulated_usage: Usage,
    ) -> Option<crate::Result<AgentEvent>> {
        self.total_usage.input_tokens += accumulated_usage.input_tokens;
        self.total_usage.output_tokens += accumulated_usage.output_tokens;
        self.metrics.add_usage(
            accumulated_usage.input_tokens,
            accumulated_usage.output_tokens,
        );
        self.metrics
            .record_model_usage(&self.cfg.config.model.primary, &accumulated_usage);

        let cost = self
            .cfg
            .budget_tracker
            .record(&self.cfg.config.model.primary, &accumulated_usage);
        self.metrics.add_cost(cost);

        if let Some(ref tenant_budget) = self.cfg.tenant_budget {
            tenant_budget.record(&self.cfg.config.model.primary, &accumulated_usage);
        }

        self.cfg
            .tool_state
            .with_session_mut(|session| {
                session.update_usage(&accumulated_usage);

                let mut content = Vec::new();
                if !self.final_text.is_empty() {
                    content.push(ContentBlock::Text {
                        text: self.final_text.clone(),
                        citations: None,
                        cache_control: None,
                    });
                }
                for tool_use in &self.pending_tool_uses {
                    content.push(ContentBlock::ToolUse(tool_use.clone()));
                }
                if !content.is_empty() {
                    session.add_assistant_message(content, Some(accumulated_usage));
                }
            })
            .await;

        if self.pending_tool_uses.is_empty() {
            self.phase = Phase::Done;
            self.metrics.execution_time_ms = self.start_time.elapsed().as_millis() as u64;

            let messages = self
                .cfg
                .tool_state
                .with_session(|session| session.to_api_messages())
                .await;

            return Some(Ok(AgentEvent::Complete(Box::new(AgentResult {
                text: self.final_text.clone(),
                usage: self.total_usage,
                tool_calls: self.metrics.tool_calls,
                iterations: self.metrics.iterations,
                stop_reason: StopReason::EndTurn,
                state: AgentState::Completed,
                metrics: self.metrics.clone(),
                session_id: self.cfg.session_id.to_string(),
                structured_output: None,
                messages,
                uuid: uuid::Uuid::new_v4().to_string(),
            }))));
        }

        self.phase = Phase::ProcessingTools { tool_index: 0 };
        None
    }

    async fn do_process_tool(&mut self, tool_index: usize) -> Option<crate::Result<AgentEvent>> {
        if tool_index >= self.pending_tool_uses.len() {
            if !self.pending_tool_results.is_empty() {
                self.finalize_tool_results().await;
            }
            self.final_text.clear();
            self.pending_tool_uses.clear();
            self.phase = Phase::StartRequest;
            return None;
        }

        let tool_use = self.pending_tool_uses[tool_index].clone();
        self.execute_tool(tool_use, tool_index).await
    }

    async fn execute_tool(
        &mut self,
        tool_use: ToolUseBlock,
        tool_index: usize,
    ) -> Option<crate::Result<AgentEvent>> {
        let pre_input = HookInput::pre_tool_use(
            &*self.cfg.session_id,
            &tool_use.name,
            tool_use.input.clone(),
        );
        let pre_output = match self
            .cfg
            .hooks
            .execute(HookEvent::PreToolUse, pre_input, &self.cfg.hook_context)
            .await
        {
            Ok(output) => output,
            Err(e) => {
                warn!(tool = %tool_use.name, error = %e, "PreToolUse hook failed");
                crate::hooks::HookOutput::allow()
            }
        };

        if !pre_output.continue_execution {
            let reason = pre_output
                .stop_reason
                .clone()
                .unwrap_or_else(|| "Blocked by hook".into());
            debug!(tool = %tool_use.name, "Tool blocked by hook");

            self.pending_tool_results
                .push(ToolResultBlock::error(&tool_use.id, reason.clone()));
            self.metrics.record_permission_denial(
                PermissionDenial::new(&tool_use.name, &tool_use.id, tool_use.input.clone())
                    .with_reason(reason.clone()),
            );
            self.phase = Phase::ProcessingTools {
                tool_index: tool_index + 1,
            };

            return Some(Ok(AgentEvent::ToolBlocked {
                id: tool_use.id,
                name: tool_use.name,
                reason,
            }));
        }

        let actual_input = pre_output.updated_input.unwrap_or(tool_use.input.clone());

        let start = Instant::now();
        let result = self
            .cfg
            .tools
            .execute(&tool_use.name, actual_input.clone())
            .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        let (output, is_error) = match &result.output {
            crate::types::ToolOutput::Success(s) => (s.clone(), false),
            crate::types::ToolOutput::SuccessBlocks(blocks) => {
                let text = blocks
                    .iter()
                    .filter_map(|b| match b {
                        crate::types::ToolOutputBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (text, false)
            }
            crate::types::ToolOutput::Error(e) => (e.to_string(), true),
            crate::types::ToolOutput::Empty => (String::new(), false),
        };

        self.metrics
            .record_tool(&tool_use.id, &tool_use.name, duration_ms, is_error);

        if let Some(ref inner_usage) = result.inner_usage {
            self.cfg
                .tool_state
                .with_session_mut(|session| {
                    session.update_usage(inner_usage);
                })
                .await;
            self.total_usage.input_tokens += inner_usage.input_tokens;
            self.total_usage.output_tokens += inner_usage.output_tokens;
            self.metrics
                .add_usage(inner_usage.input_tokens, inner_usage.output_tokens);
            let inner_model = result.inner_model.as_deref().unwrap_or("claude-haiku-4-5");
            self.metrics.record_model_usage(inner_model, inner_usage);
            let inner_cost = self.cfg.budget_tracker.record(inner_model, inner_usage);
            self.metrics.add_cost(inner_cost);
        }

        if is_error {
            let failure_input = HookInput::post_tool_use_failure(
                &*self.cfg.session_id,
                &tool_use.name,
                result.error_message(),
            );
            if let Err(e) = self
                .cfg
                .hooks
                .execute(
                    HookEvent::PostToolUseFailure,
                    failure_input,
                    &self.cfg.hook_context,
                )
                .await
            {
                warn!(tool = %tool_use.name, error = %e, "PostToolUseFailure hook failed");
            }
        } else {
            let post_input = HookInput::post_tool_use(
                &*self.cfg.session_id,
                &tool_use.name,
                result.output.clone(),
            );
            if let Err(e) = self
                .cfg
                .hooks
                .execute(HookEvent::PostToolUse, post_input, &self.cfg.hook_context)
                .await
            {
                warn!(tool = %tool_use.name, error = %e, "PostToolUse hook failed");
            }
        }

        if let Some(file_path) = extract_file_path(&tool_use.name, &actual_input)
            && let Some(ref orchestrator) = self.cfg.orchestrator
        {
            let orch = orchestrator.read().await;
            let path = Path::new(&file_path);
            if orch.has_matching_rules(path).await {
                let dynamic_ctx = orch.build_dynamic_context(Some(path)).await;
                if !dynamic_ctx.is_empty() {
                    self.dynamic_rules = dynamic_ctx;
                }
            }
        }

        self.pending_tool_results
            .push(ToolResultBlock::from_tool_result(&tool_use.id, &result));
        self.phase = Phase::ProcessingTools {
            tool_index: tool_index + 1,
        };

        Some(Ok(AgentEvent::ToolComplete {
            id: tool_use.id,
            name: tool_use.name,
            output,
            is_error,
            duration_ms,
        }))
    }

    async fn finalize_tool_results(&mut self) {
        let results = std::mem::take(&mut self.pending_tool_results);
        let max_tokens = context_window::for_model(&self.cfg.config.model.primary);

        self.cfg
            .tool_state
            .with_session_mut(|session| {
                session.add_tool_results(results);
            })
            .await;

        let should_compact = self
            .cfg
            .tool_state
            .with_session(|session| {
                self.cfg.config.execution.auto_compact
                    && session.should_compact(
                        max_tokens,
                        self.cfg.config.execution.compact_threshold,
                        self.cfg.config.execution.compact_keep_messages,
                    )
            })
            .await;

        if should_compact {
            let compact_result = self
                .cfg
                .tool_state
                .compact(
                    &self.cfg.client,
                    self.cfg.config.execution.compact_keep_messages,
                )
                .await;

            if let Ok(CompactResult::Compacted { .. }) = compact_result {
                self.metrics.record_compaction();
                let state_sections = collect_compaction_state(&self.cfg.tools).await;
                if !state_sections.is_empty() {
                    self.cfg
                        .tool_state
                        .with_session_mut(|session| {
                            session.add_user_message(format!(
                                "<system-reminder>\n# State preserved after compaction\n\n{}\n</system-reminder>",
                                state_sections.join("\n\n")
                            ));
                        })
                        .await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_transitions() {
        assert!(matches!(Phase::StartRequest, Phase::StartRequest));
        assert!(matches!(Phase::Done, Phase::Done));
    }

    #[test]
    fn test_stream_poll_result_variants() {
        let event = StreamPollResult::Event(Ok(AgentEvent::Text("test".into())));
        assert!(matches!(event, StreamPollResult::Event(_)));

        let cont = StreamPollResult::Continue;
        assert!(matches!(cont, StreamPollResult::Continue));

        let ended = StreamPollResult::StreamEnded;
        assert!(matches!(ended, StreamPollResult::StreamEnded));
    }
}
