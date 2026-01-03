//! Agent streaming execution.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use futures::Stream;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, warn};

use super::events::{AgentEvent, AgentResult};
use super::execution::extract_file_path;
use super::executor::Agent;
use super::request::RequestBuilder;
use super::state_formatter::collect_compaction_state;
use super::{AgentConfig, AgentMetrics, AgentState, ConversationContext};
use crate::budget::{BudgetTracker, TenantBudget};
use crate::context::PromptOrchestrator;
use crate::hooks::{HookContext, HookEvent, HookInput, HookManager};
use crate::types::{
    CompactResult, ContentBlock, Message, PermissionDenial, StopReason, ToolResultBlock,
    ToolUseBlock,
};
use crate::{Client, ToolRegistry};

impl Agent {
    pub async fn execute_stream(
        &self,
        prompt: &str,
    ) -> crate::Result<impl Stream<Item = crate::Result<AgentEvent>>> {
        use futures::stream;

        let context = Arc::new(Mutex::new(ConversationContext::new()));
        {
            let mut history = context.lock().await;
            history.push(Message::user(prompt));
        }

        let request_builder = RequestBuilder::new(&self.config, Arc::clone(&self.tools));

        let event_stream = stream::unfold(
            StreamState::new(
                context,
                self.client.clone(),
                self.config.clone(),
                Arc::clone(&self.tools),
                self.hooks.clone(),
                self.hook_context(),
                request_builder,
                self.orchestrator.clone(),
                self.session_id.clone(),
                self.budget_tracker.clone(),
                self.tenant_budget.clone(),
            ),
            |mut state| async move { state.next_event().await.map(|event| (event, state)) },
        );

        Ok(event_stream)
    }
}

pub(crate) struct StreamState {
    context: Arc<Mutex<ConversationContext>>,
    client: Client,
    config: AgentConfig,
    tools: Arc<ToolRegistry>,
    hooks: HookManager,
    hook_context: HookContext,
    request_builder: RequestBuilder,
    dynamic_rules: String,
    orchestrator: Option<Arc<RwLock<PromptOrchestrator>>>,
    metrics: AgentMetrics,
    start_time: Instant,
    pending_events: VecDeque<crate::Result<AgentEvent>>,
    pending_tool_results: Vec<ToolResultBlock>,
    pending_tool_uses: Vec<ToolUseBlock>,
    final_text: String,
    done: bool,
    session_id: String,
    budget_tracker: BudgetTracker,
    tenant_budget: Option<Arc<TenantBudget>>,
}

impl StreamState {
    #[allow(clippy::too_many_arguments)]
    fn new(
        context: Arc<Mutex<ConversationContext>>,
        client: Client,
        config: AgentConfig,
        tools: Arc<ToolRegistry>,
        hooks: HookManager,
        hook_context: HookContext,
        request_builder: RequestBuilder,
        orchestrator: Option<Arc<RwLock<PromptOrchestrator>>>,
        session_id: String,
        budget_tracker: BudgetTracker,
        tenant_budget: Option<Arc<TenantBudget>>,
    ) -> Self {
        Self {
            context,
            client,
            config,
            tools,
            hooks,
            hook_context,
            request_builder,
            dynamic_rules: String::new(),
            orchestrator,
            metrics: AgentMetrics::default(),
            start_time: Instant::now(),
            pending_events: VecDeque::new(),
            pending_tool_results: Vec::new(),
            pending_tool_uses: Vec::new(),
            final_text: String::new(),
            done: false,
            session_id,
            budget_tracker,
            tenant_budget,
        }
    }

    async fn next_event(&mut self) -> Option<crate::Result<AgentEvent>> {
        if let Some(event) = self.pending_events.pop_front() {
            return Some(event);
        }

        if self.done {
            return None;
        }

        if !self.pending_tool_uses.is_empty() {
            return self.process_tool_use().await;
        }

        if self.budget_tracker.should_stop() {
            self.done = true;
            let status = self.budget_tracker.check();
            return Some(Err(crate::Error::BudgetExceeded {
                used: status.used(),
                limit: self.config.budget.max_cost_usd.unwrap_or(0.0),
            }));
        }
        if let Some(ref tenant_budget) = self.tenant_budget
            && tenant_budget.should_stop()
        {
            self.done = true;
            return Some(Err(crate::Error::BudgetExceeded {
                used: tenant_budget.used_cost_usd(),
                limit: tenant_budget.max_cost_usd(),
            }));
        }

        self.metrics.iterations += 1;
        if self.metrics.iterations > self.config.execution.max_iterations {
            return self.complete_with_max_iterations().await;
        }

        self.fetch_and_process_response().await
    }

    async fn process_tool_use(&mut self) -> Option<crate::Result<AgentEvent>> {
        let tool_use = self.pending_tool_uses.remove(0);

        let pre_input =
            HookInput::pre_tool_use(&self.session_id, &tool_use.name, tool_use.input.clone());
        let pre_output = match self
            .hooks
            .execute(HookEvent::PreToolUse, pre_input, &self.hook_context)
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
            self.pending_events.push_back(Ok(AgentEvent::ToolStart {
                id: tool_use.id.clone(),
                name: tool_use.name.clone(),
                input: tool_use.input.clone(),
            }));
            self.pending_events.push_back(Ok(AgentEvent::ToolEnd {
                id: tool_use.id.clone(),
                output: reason.clone(),
                is_error: true,
            }));
            self.pending_tool_results
                .push(ToolResultBlock::error(&tool_use.id, reason.clone()));
            self.metrics.record_permission_denial(
                PermissionDenial::new(&tool_use.name, &tool_use.id, tool_use.input.clone())
                    .with_reason(reason),
            );
            return self.pending_events.pop_front();
        }

        let actual_input = pre_output.updated_input.unwrap_or(tool_use.input.clone());

        self.pending_events.push_back(Ok(AgentEvent::ToolStart {
            id: tool_use.id.clone(),
            name: tool_use.name.clone(),
            input: actual_input.clone(),
        }));

        let start = Instant::now();
        let result = self
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
            .record_tool(&tool_use.name, duration_ms, is_error);

        if let Some(ref inner_usage) = result.inner_usage {
            let mut history = self.context.lock().await;
            history.update_usage(*inner_usage);
            self.metrics
                .add_usage(inner_usage.input_tokens, inner_usage.output_tokens);
            let inner_model = result.inner_model.as_deref().unwrap_or("claude-haiku-4-5");
            self.metrics.record_model_usage(inner_model, inner_usage);
            let inner_cost = self.budget_tracker.record(inner_model, inner_usage);
            self.metrics.add_cost(inner_cost);
        }

        if is_error {
            let failure_input = HookInput::post_tool_use_failure(
                &self.session_id,
                &tool_use.name,
                result.error_message(),
            );
            if let Err(e) = self
                .hooks
                .execute(
                    HookEvent::PostToolUseFailure,
                    failure_input,
                    &self.hook_context,
                )
                .await
            {
                warn!(tool = %tool_use.name, error = %e, "PostToolUseFailure hook failed");
            }
        } else {
            let post_input =
                HookInput::post_tool_use(&self.session_id, &tool_use.name, result.output.clone());
            if let Err(e) = self
                .hooks
                .execute(HookEvent::PostToolUse, post_input, &self.hook_context)
                .await
            {
                warn!(tool = %tool_use.name, error = %e, "PostToolUse hook failed");
            }
        }

        if let Some(file_path) = extract_file_path(&tool_use.name, &actual_input)
            && let Some(ref orchestrator) = self.orchestrator
        {
            let orch = orchestrator.read().await;
            let path = Path::new(&file_path);
            let rules = orch.rules_engine().find_matching(path);
            if !rules.is_empty() {
                let rule_names: Vec<String> = rules.iter().map(|r| r.name.clone()).collect();
                let dynamic_ctx = orch.build_dynamic_context(Some(path)).await;
                if !dynamic_ctx.is_empty() {
                    self.dynamic_rules = dynamic_ctx;
                }
                self.pending_events
                    .push_back(Ok(AgentEvent::RulesActivated {
                        file_path,
                        rule_names,
                    }));
            }
        }

        self.pending_events.push_back(Ok(AgentEvent::ToolEnd {
            id: tool_use.id.clone(),
            output: output.clone(),
            is_error,
        }));

        self.pending_tool_results
            .push(ToolResultBlock::from_tool_result(&tool_use.id, result));

        if self.pending_tool_uses.is_empty() && !self.pending_tool_results.is_empty() {
            self.finalize_tool_results().await;
        }

        self.pending_events.pop_front()
    }

    async fn finalize_tool_results(&mut self) {
        let results = std::mem::take(&mut self.pending_tool_results);
        let mut history = self.context.lock().await;
        history.push(Message::tool_results(results));

        let used_tokens = history.estimated_tokens() as u64;
        let max_tokens = 200_000u64;
        self.pending_events.push_back(Ok(AgentEvent::ContextUpdate {
            used_tokens,
            max_tokens,
        }));

        if self.config.execution.auto_compact
            && history.should_compact(
                max_tokens as usize,
                self.config.execution.compact_threshold,
                self.config.execution.compact_keep_messages,
            )
        {
            self.pending_events
                .push_back(Ok(AgentEvent::CompactStarted));
            let previous_tokens = history.estimated_tokens() as u64;

            if let Ok(CompactResult::Compacted { .. }) = history
                .compact(&self.client, self.config.execution.compact_keep_messages)
                .await
            {
                let current_tokens = history.estimated_tokens() as u64;
                self.pending_events
                    .push_back(Ok(AgentEvent::CompactCompleted {
                        previous_tokens,
                        current_tokens,
                    }));
                self.metrics.record_compaction();

                let state_sections = collect_compaction_state(&self.tools).await;
                if !state_sections.is_empty() {
                    history.push(Message::user(format!(
                        "<system-reminder>\n# State preserved after compaction\n\n{}\n</system-reminder>",
                        state_sections.join("\n\n")
                    )));
                }
            }
        }
    }

    async fn complete_with_max_iterations(&mut self) -> Option<crate::Result<AgentEvent>> {
        self.done = true;
        self.metrics.execution_time_ms = self.start_time.elapsed().as_millis() as u64;
        let history = self.context.lock().await;
        Some(Ok(AgentEvent::Complete(Box::new(AgentResult {
            text: self.final_text.clone(),
            usage: *history.total_usage(),
            tool_calls: self.metrics.tool_calls,
            iterations: self.metrics.iterations - 1,
            stop_reason: StopReason::MaxTokens,
            state: AgentState::Completed,
            metrics: self.metrics.clone(),
            session_id: self.session_id.clone(),
            structured_output: None,
            messages: history.messages().to_vec(),
            uuid: uuid::Uuid::new_v4().to_string(),
        }))))
    }

    async fn fetch_and_process_response(&mut self) -> Option<crate::Result<AgentEvent>> {
        let request = {
            let history = self.context.lock().await;
            self.request_builder
                .build(history.messages().to_vec(), &self.dynamic_rules)
        };

        let response = match self.client.send(request.clone()).await {
            Ok(r) => r,
            Err(e) if e.is_unauthorized() => {
                if let Err(refresh_err) = self.client.refresh_credentials().await {
                    self.done = true;
                    return Some(Err(refresh_err));
                }
                match self.client.send(request).await {
                    Ok(r) => r,
                    Err(e) => {
                        self.done = true;
                        return Some(Err(e));
                    }
                }
            }
            Err(e) => {
                self.done = true;
                return Some(Err(e));
            }
        };

        self.metrics.record_api_call();
        self.metrics
            .add_usage(response.usage.input_tokens, response.usage.output_tokens);
        self.metrics
            .record_model_usage(&self.config.model.primary, &response.usage);

        let cost = self
            .budget_tracker
            .record(&self.config.model.primary, &response.usage);
        self.metrics.add_cost(cost);
        if let Some(ref tenant_budget) = self.tenant_budget {
            tenant_budget.record(&self.config.model.primary, &response.usage);
        }

        {
            let mut history = self.context.lock().await;
            history.update_usage(response.usage);
        }

        let mut text_content = String::new();
        let mut tool_uses = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text, .. } => {
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
            let mut history = self.context.lock().await;
            history.push(Message {
                role: crate::types::Role::Assistant,
                content: response.content.clone(),
            });
        }

        if response.wants_tool_use() && !tool_uses.is_empty() {
            self.pending_tool_uses = tool_uses;
        } else {
            self.done = true;
            self.metrics.execution_time_ms = self.start_time.elapsed().as_millis() as u64;
            let history = self.context.lock().await;
            self.pending_events
                .push_back(Ok(AgentEvent::Complete(Box::new(AgentResult {
                    text: self.final_text.clone(),
                    usage: *history.total_usage(),
                    tool_calls: self.metrics.tool_calls,
                    iterations: self.metrics.iterations,
                    stop_reason: response.stop_reason.unwrap_or(StopReason::EndTurn),
                    state: AgentState::Completed,
                    metrics: self.metrics.clone(),
                    session_id: self.session_id.clone(),
                    structured_output: None,
                    messages: history.messages().to_vec(),
                    uuid: uuid::Uuid::new_v4().to_string(),
                }))));
        }

        self.pending_events.pop_front()
    }
}
