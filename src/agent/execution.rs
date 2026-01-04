//! Agent execution logic with session-based context management.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

use super::common::BudgetContext;
use super::events::AgentResult;
use super::executor::Agent;
use super::request::RequestBuilder;
use super::state_formatter::collect_compaction_state;
use super::{AgentMetrics, AgentState};
use crate::context::PromptOrchestrator;
use crate::hooks::{HookContext, HookEvent, HookInput};
use crate::session::ExecutionGuard;
use crate::types::{
    CompactResult, ContentBlock, Message, PermissionDenial, StopReason, ToolResultBlock, Usage,
    context_window,
};

impl Agent {
    async fn handle_compaction<'a>(
        &self,
        _guard: &ExecutionGuard<'a>,
        hook_ctx: &HookContext,
        metrics: &mut AgentMetrics,
    ) {
        let pre_compact_input = HookInput::pre_compact(&*self.session_id);
        if let Err(e) = self
            .hooks
            .execute(HookEvent::PreCompact, pre_compact_input, hook_ctx)
            .await
        {
            warn!(error = %e, "PreCompact hook failed");
        }

        debug!("Compacting session context");
        let compact_result = self
            .state
            .compact(&self.client, self.config.execution.compact_keep_messages)
            .await;

        match compact_result {
            Ok(CompactResult::Compacted { saved_tokens, .. }) => {
                info!(saved_tokens, "Session context compacted");
                metrics.record_compaction();

                let state_sections = collect_compaction_state(&self.tools).await;
                if !state_sections.is_empty() {
                    self.state
                        .with_session_mut(|session| {
                            session.add_user_message(format!(
                                "<system-reminder>\n# State preserved after compaction\n\n{}\n</system-reminder>",
                                state_sections.join("\n\n")
                            ));
                        })
                        .await;
                }
            }
            Ok(CompactResult::NotNeeded | CompactResult::Skipped { .. }) => {
                debug!("Compaction skipped or not needed");
            }
            Err(e) => {
                warn!(error = %e, "Session compaction failed");
            }
        }
    }

    fn check_budget(&self) -> crate::Result<()> {
        BudgetContext {
            tracker: &self.budget_tracker,
            tenant: self.tenant_budget.as_deref(),
            config: &self.config.budget,
        }
        .check()
    }

    pub async fn execute(&self, prompt: &str) -> crate::Result<AgentResult> {
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
            return self.wait_for_execution(timeout).await;
        }

        tokio::time::timeout(timeout, self.execute_inner(prompt))
            .await
            .map_err(|_| crate::Error::Timeout(timeout))?
    }

    async fn wait_for_execution(
        &self,
        timeout: std::time::Duration,
    ) -> crate::Result<AgentResult> {
        let start = Instant::now();
        loop {
            if start.elapsed() > timeout {
                return Err(crate::Error::Timeout(timeout));
            }

            if !self.state.is_executing()
                && let Some(merged) = self.state.dequeue_or_merge().await
            {
                return self.execute_inner(&merged.content).await;
            }

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    pub async fn execute_with_messages(
        &self,
        previous_messages: Vec<Message>,
        prompt: &str,
    ) -> crate::Result<AgentResult> {
        let context_summary = previous_messages
            .iter()
            .filter_map(|m| {
                m.content
                    .iter()
                    .filter_map(|b| match b {
                        ContentBlock::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .next()
            })
            .collect::<Vec<_>>()
            .join("\n---\n");

        let enriched_prompt = if context_summary.is_empty() {
            prompt.to_string()
        } else {
            format!(
                "Previous conversation context:\n{}\n\nContinue with: {}",
                context_summary, prompt
            )
        };

        self.execute(&enriched_prompt).await
    }

    #[instrument(skip(self, prompt), fields(session_id = %self.session_id))]
    async fn execute_inner(&self, prompt: &str) -> crate::Result<AgentResult> {
        let guard = self.state.acquire_execution().await;
        let execution_start = Instant::now();
        let hook_ctx = self.hook_context();

        let session_start_input = HookInput::session_start(&*self.session_id);
        if let Err(e) = self
            .hooks
            .execute(HookEvent::SessionStart, session_start_input, &hook_ctx)
            .await
        {
            warn!(error = %e, "SessionStart hook failed");
        }

        let final_prompt = if let Some(merged) = self.state.dequeue_or_merge().await {
            format!("{}\n{}", prompt, merged.content)
        } else {
            prompt.to_string()
        };

        let prompt_input = HookInput::user_prompt_submit(&*self.session_id, &final_prompt);
        let prompt_output = self
            .hooks
            .execute(HookEvent::UserPromptSubmit, prompt_input, &hook_ctx)
            .await?;

        if !prompt_output.continue_execution {
            let session_end_input = HookInput::session_end(&*self.session_id);
            if let Err(e) = self
                .hooks
                .execute(HookEvent::SessionEnd, session_end_input, &hook_ctx)
                .await
            {
                warn!(error = %e, "SessionEnd hook failed");
            }
            return Err(crate::Error::Permission(
                prompt_output
                    .stop_reason
                    .unwrap_or_else(|| "Blocked by hook".into()),
            ));
        }

        self.state
            .with_session_mut(|session| {
                session.add_user_message(&final_prompt);
            })
            .await;

        let mut metrics = AgentMetrics::default();
        let mut final_text = String::new();
        let mut final_stop_reason = StopReason::EndTurn;
        let mut dynamic_rules_context = String::new();
        let mut total_usage = Usage::default();

        let request_builder = RequestBuilder::new(&self.config, Arc::clone(&self.tools));
        let max_tokens = context_window::for_model(&self.config.model.primary);

        info!(prompt_len = final_prompt.len(), "Starting agent execution");

        loop {
            metrics.iterations += 1;
            if metrics.iterations > self.config.execution.max_iterations {
                warn!(
                    max = self.config.execution.max_iterations,
                    "Max iterations reached"
                );
                break;
            }

            self.check_budget()?;

            debug!(iteration = metrics.iterations, "Starting iteration");

            let cache_messages = self.config.cache.enabled && self.config.cache.message_cache;
            let messages = self
                .state
                .with_session(|session| session.to_api_messages_with_cache(cache_messages))
                .await;

            let api_start = Instant::now();
            let request = request_builder.build(messages, &dynamic_rules_context);
            let response = self.client.send_with_auth_retry(request).await?;
            let api_duration_ms = api_start.elapsed().as_millis() as u64;
            metrics.record_api_call_with_timing(api_duration_ms);
            debug!(api_time_ms = api_duration_ms, "API call completed");

            self.state
                .with_session_mut(|session| {
                    session.update_usage(&response.usage);
                })
                .await;

            total_usage.input_tokens += response.usage.input_tokens;
            total_usage.output_tokens += response.usage.output_tokens;
            metrics.add_usage_with_cache(&response.usage);
            metrics.record_model_usage(&self.config.model.primary, &response.usage);

            if let Some(ref server_usage) = response.usage.server_tool_use {
                metrics.update_server_tool_use_from_api(server_usage);
            }

            let cost = self
                .budget_tracker
                .record(&self.config.model.primary, &response.usage);
            metrics.add_cost(cost);
            if let Some(ref tenant_budget) = self.tenant_budget {
                tenant_budget.record(&self.config.model.primary, &response.usage);
            }

            final_text = response.text();
            final_stop_reason = response.stop_reason.unwrap_or(StopReason::EndTurn);

            self.state
                .with_session_mut(|session| {
                    session.add_assistant_message(response.content.clone(), Some(response.usage));
                })
                .await;

            if !response.wants_tool_use() {
                debug!("No tool use requested, ending loop");
                break;
            }

            let tool_uses = response.tool_uses();
            let hook_ctx = self.hook_context();

            let mut prepared = Vec::with_capacity(tool_uses.len());
            let mut blocked = Vec::with_capacity(tool_uses.len());

            for tool_use in &tool_uses {
                let pre_input = HookInput::pre_tool_use(
                    &*self.session_id,
                    &tool_use.name,
                    tool_use.input.clone(),
                );
                let pre_output = self
                    .hooks
                    .execute(HookEvent::PreToolUse, pre_input, &hook_ctx)
                    .await?;

                if !pre_output.continue_execution {
                    debug!(tool = %tool_use.name, "Tool blocked by hook");
                    let reason = pre_output
                        .stop_reason
                        .clone()
                        .unwrap_or_else(|| "Blocked by hook".into());
                    blocked.push(ToolResultBlock::error(&tool_use.id, reason.clone()));
                    metrics.record_permission_denial(
                        PermissionDenial::new(&tool_use.name, &tool_use.id, tool_use.input.clone())
                            .with_reason(reason),
                    );
                } else {
                    let input = pre_output.updated_input.unwrap_or(tool_use.input.clone());
                    prepared.push((tool_use.id.clone(), tool_use.name.clone(), input));
                }
            }

            let tool_futures = prepared.into_iter().map(|(id, name, input)| {
                let tools = &self.tools;
                async move {
                    let start = Instant::now();
                    let result = tools.execute(&name, input.clone()).await;
                    let duration_ms = start.elapsed().as_millis() as u64;
                    (id, name, input, result, duration_ms)
                }
            });

            let parallel_results: Vec<_> = futures::future::join_all(tool_futures).await;

            let all_non_retryable = !parallel_results.is_empty()
                && parallel_results
                    .iter()
                    .all(|(_, _, _, result, _)| result.is_non_retryable());

            let mut results = blocked;
            for (id, name, input, result, duration_ms) in parallel_results {
                let is_error = result.is_error();
                debug!(tool = %name, duration_ms, is_error, "Tool execution completed");
                metrics.record_tool(&id, &name, duration_ms, is_error);

                if let Some(ref inner_usage) = result.inner_usage {
                    self.state
                        .with_session_mut(|session| {
                            session.update_usage(inner_usage);
                        })
                        .await;
                    total_usage.input_tokens += inner_usage.input_tokens;
                    total_usage.output_tokens += inner_usage.output_tokens;
                    metrics.add_usage(inner_usage.input_tokens, inner_usage.output_tokens);
                    let inner_model = result.inner_model.as_deref().unwrap_or("claude-haiku-4-5");
                    metrics.record_model_usage(inner_model, inner_usage);

                    let inner_cost = self.budget_tracker.record(inner_model, inner_usage);
                    metrics.add_cost(inner_cost);

                    debug!(
                        tool = %name,
                        model = %inner_model,
                        input_tokens = inner_usage.input_tokens,
                        output_tokens = inner_usage.output_tokens,
                        cost_usd = inner_cost,
                        "Accumulated inner usage from tool"
                    );
                }

                if let Some(file_path) = extract_file_path(&name, &input)
                    && let Some(ref orchestrator) = self.orchestrator
                {
                    let new_rules = activate_rules_for_file(orchestrator, &file_path).await;
                    if !new_rules.is_empty() {
                        dynamic_rules_context =
                            build_dynamic_rules_context(orchestrator, &file_path).await;
                        debug!(rules = ?new_rules, "Activated rules for file");
                    }
                }

                if is_error {
                    let failure_input = HookInput::post_tool_use_failure(
                        &*self.session_id,
                        &name,
                        result.error_message(),
                    );
                    if let Err(e) = self
                        .hooks
                        .execute(HookEvent::PostToolUseFailure, failure_input, &hook_ctx)
                        .await
                    {
                        warn!(tool = %name, error = %e, "PostToolUseFailure hook failed");
                    }
                } else {
                    let post_input =
                        HookInput::post_tool_use(&*self.session_id, &name, result.output.clone());
                    if let Err(e) = self
                        .hooks
                        .execute(HookEvent::PostToolUse, post_input, &hook_ctx)
                        .await
                    {
                        warn!(tool = %name, error = %e, "PostToolUse hook failed");
                    }
                }
                results.push(ToolResultBlock::from_tool_result(&id, result));
            }

            self.state
                .with_session_mut(|session| {
                    session.add_tool_results(results);
                })
                .await;

            if all_non_retryable {
                warn!("All tool calls failed with non-retryable errors, ending execution");
                break;
            }

            let should_compact = self
                .state
                .with_session(|session| {
                    self.config.execution.auto_compact
                        && session.should_compact(
                            max_tokens,
                            self.config.execution.compact_threshold,
                            self.config.execution.compact_keep_messages,
                        )
                })
                .await;

            if should_compact {
                self.handle_compaction(&guard, &hook_ctx, &mut metrics)
                    .await;
            }
        }

        metrics.execution_time_ms = execution_start.elapsed().as_millis() as u64;

        let stop_input = HookInput::stop(&*self.session_id);
        if let Err(e) = self
            .hooks
            .execute(HookEvent::Stop, stop_input, &hook_ctx)
            .await
        {
            warn!(error = %e, "Stop hook failed");
        }

        let session_end_input = HookInput::session_end(&*self.session_id);
        if let Err(e) = self
            .hooks
            .execute(HookEvent::SessionEnd, session_end_input, &hook_ctx)
            .await
        {
            warn!(error = %e, "SessionEnd hook failed");
        }

        info!(
            iterations = metrics.iterations,
            tool_calls = metrics.tool_calls,
            api_calls = metrics.api_calls,
            total_tokens = metrics.total_tokens(),
            execution_time_ms = metrics.execution_time_ms,
            "Agent execution completed"
        );

        let messages = self
            .state
            .with_session(|session| session.to_api_messages())
            .await;

        drop(guard);

        Ok(AgentResult {
            text: final_text,
            usage: total_usage,
            tool_calls: metrics.tool_calls,
            iterations: metrics.iterations,
            stop_reason: final_stop_reason,
            state: AgentState::Completed,
            metrics,
            session_id: self.session_id.to_string(),
            structured_output: None,
            messages,
            uuid: uuid::Uuid::new_v4().to_string(),
        })
    }

    pub(crate) fn hook_context(&self) -> HookContext {
        HookContext::new(&*self.session_id)
            .with_cwd(self.config.working_dir.clone().unwrap_or_default())
            .with_env(self.config.security.env.clone())
    }
}

pub(crate) fn extract_file_path(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    match tool_name {
        "Read" | "Write" | "Edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(String::from),
        "Glob" | "Grep" => input.get("path").and_then(|v| v.as_str()).map(String::from),
        _ => None,
    }
}

pub(crate) async fn activate_rules_for_file(
    orchestrator: &Arc<RwLock<PromptOrchestrator>>,
    file_path: &str,
) -> Vec<String> {
    let orch = orchestrator.read().await;
    let path = Path::new(file_path);
    let rules = orch.rules_engine().find_matching(path);
    rules.iter().map(|r| r.name.clone()).collect()
}

pub(crate) async fn build_dynamic_rules_context(
    orchestrator: &Arc<RwLock<PromptOrchestrator>>,
    file_path: &str,
) -> String {
    let orch = orchestrator.read().await;
    let path = Path::new(file_path);
    orch.build_dynamic_context(Some(path)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_file_path() {
        let input = serde_json::json!({"file_path": "/src/lib.rs"});
        assert_eq!(
            extract_file_path("Read", &input),
            Some("/src/lib.rs".to_string())
        );

        let input = serde_json::json!({"path": "/src"});
        assert_eq!(extract_file_path("Glob", &input), Some("/src".to_string()));

        let input = serde_json::json!({"command": "ls"});
        assert_eq!(extract_file_path("Bash", &input), None);
    }

    #[test]
    fn test_extract_file_path_all_tools() {
        let file_input = serde_json::json!({"file_path": "/test/file.rs"});
        let path_input = serde_json::json!({"path": "/test/dir"});

        assert_eq!(
            extract_file_path("Read", &file_input),
            Some("/test/file.rs".to_string())
        );
        assert_eq!(
            extract_file_path("Write", &file_input),
            Some("/test/file.rs".to_string())
        );
        assert_eq!(
            extract_file_path("Edit", &file_input),
            Some("/test/file.rs".to_string())
        );

        assert_eq!(
            extract_file_path("Glob", &path_input),
            Some("/test/dir".to_string())
        );
        assert_eq!(
            extract_file_path("Grep", &path_input),
            Some("/test/dir".to_string())
        );

        assert_eq!(extract_file_path("WebFetch", &file_input), None);
        assert_eq!(extract_file_path("Task", &file_input), None);
    }

    #[test]
    fn test_extract_file_path_missing_field() {
        let empty = serde_json::json!({});
        assert_eq!(extract_file_path("Read", &empty), None);
        assert_eq!(extract_file_path("Glob", &empty), None);

        let wrong_field = serde_json::json!({"other": "value"});
        assert_eq!(extract_file_path("Read", &wrong_field), None);
        assert_eq!(extract_file_path("Glob", &wrong_field), None);
    }

    #[test]
    fn test_extract_file_path_non_string() {
        let input = serde_json::json!({"file_path": 123});
        assert_eq!(extract_file_path("Read", &input), None);

        let input = serde_json::json!({"path": null});
        assert_eq!(extract_file_path("Glob", &input), None);
    }
}
