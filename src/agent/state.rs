//! Agent state management.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    #[default]
    Initializing,
    Running,
    WaitingForToolResults,
    WaitingForUserInput,
    PlanMode,
    Completed,
    Failed,
    Cancelled,
}

impl AgentState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn is_waiting(&self) -> bool {
        matches!(
            self,
            Self::WaitingForToolResults | Self::WaitingForUserInput | Self::PlanMode
        )
    }

    pub fn can_continue(&self) -> bool {
        matches!(self, Self::Running | Self::Initializing)
    }
}

use crate::types::{ModelUsage, PermissionDenial, ServerToolUse, ServerToolUseUsage, Usage};

#[derive(Debug, Clone, Default)]
pub struct AgentMetrics {
    pub iterations: usize,
    pub tool_calls: usize,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub execution_time_ms: u64,
    pub errors: usize,
    pub compactions: usize,
    pub api_calls: usize,
    pub total_cost_usd: f64,
    pub tool_stats: std::collections::HashMap<String, ToolStats>,
    pub model_usage: std::collections::HashMap<String, ModelUsage>,
    pub server_tool_use: ServerToolUse,
    pub permission_denials: Vec<PermissionDenial>,
    pub api_time_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ToolStats {
    pub calls: usize,
    pub total_time_ms: u64,
    pub errors: usize,
}

impl AgentMetrics {
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }

    pub fn add_usage(&mut self, input: u32, output: u32) {
        self.input_tokens += input;
        self.output_tokens += output;
    }

    pub fn add_usage_with_cache(&mut self, usage: &Usage) {
        self.input_tokens += usage.input_tokens;
        self.output_tokens += usage.output_tokens;
        self.cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
        self.cache_creation_tokens += usage.cache_creation_input_tokens.unwrap_or(0);
    }

    pub fn cache_hit_rate(&self) -> f64 {
        if self.input_tokens == 0 {
            return 0.0;
        }
        self.cache_read_tokens as f64 / self.input_tokens as f64
    }

    pub fn cache_tokens_saved(&self) -> u32 {
        (self.cache_read_tokens as f64 * 0.9) as u32
    }

    pub fn add_cost(&mut self, cost: f64) {
        self.total_cost_usd += cost;
    }

    pub fn record_tool(&mut self, name: &str, duration_ms: u64, is_error: bool) {
        self.tool_calls += 1;
        let stats = self.tool_stats.entry(name.to_string()).or_default();
        stats.calls += 1;
        stats.total_time_ms += duration_ms;
        if is_error {
            stats.errors += 1;
            self.errors += 1;
        }
    }

    pub fn record_api_call(&mut self) {
        self.api_calls += 1;
    }

    pub fn record_compaction(&mut self) {
        self.compactions += 1;
    }

    pub fn avg_tool_time_ms(&self) -> f64 {
        if self.tool_calls == 0 {
            return 0.0;
        }
        let total: u64 = self.tool_stats.values().map(|s| s.total_time_ms).sum();
        total as f64 / self.tool_calls as f64
    }

    /// Record usage for a specific model.
    ///
    /// This enables per-model cost tracking like CLI's modelUsage field.
    pub fn record_model_usage(&mut self, model: &str, usage: &Usage) {
        let entry = self.model_usage.entry(model.to_string()).or_default();
        entry.add_usage(usage, model);
    }

    /// Record an API call with timing information.
    pub fn record_api_call_with_timing(&mut self, duration_ms: u64) {
        self.api_calls += 1;
        self.api_time_ms += duration_ms;
    }

    /// Update server_tool_use from API response.
    ///
    /// This is for server-side tools executed by the API (e.g., Anthropic's
    /// server-side RAG). Not to be confused with local tool usage.
    pub fn update_server_tool_use(&mut self, server_tool_use: ServerToolUse) {
        self.server_tool_use.web_search_requests += server_tool_use.web_search_requests;
        self.server_tool_use.web_fetch_requests += server_tool_use.web_fetch_requests;
    }

    /// Update server_tool_use from API response's usage.server_tool_use field.
    ///
    /// This parses the server tool usage directly from the API response.
    pub fn update_server_tool_use_from_api(&mut self, usage: &ServerToolUseUsage) {
        self.server_tool_use.add_from_usage(usage);
    }

    /// Record a permission denial.
    pub fn record_permission_denial(&mut self, denial: PermissionDenial) {
        self.permission_denials.push(denial);
    }

    /// Get the total cost across all models.
    pub fn total_model_cost(&self) -> f64 {
        self.model_usage.values().map(|m| m.cost_usd).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_state() {
        assert!(AgentState::Completed.is_terminal());
        assert!(AgentState::Failed.is_terminal());
        assert!(!AgentState::Running.is_terminal());

        assert!(AgentState::WaitingForUserInput.is_waiting());
        assert!(AgentState::PlanMode.is_waiting());
        assert!(!AgentState::Running.is_waiting());

        assert!(AgentState::Running.can_continue());
        assert!(!AgentState::Completed.can_continue());
    }

    #[test]
    fn test_agent_metrics() {
        let mut metrics = AgentMetrics::default();
        metrics.add_usage(100, 50);
        metrics.add_usage(200, 100);

        assert_eq!(metrics.input_tokens, 300);
        assert_eq!(metrics.output_tokens, 150);
        assert_eq!(metrics.total_tokens(), 450);
    }

    #[test]
    fn test_agent_metrics_tool_recording() {
        let mut metrics = AgentMetrics::default();
        metrics.record_tool("Read", 50, false);
        metrics.record_tool("Read", 30, false);
        metrics.record_tool("Bash", 100, true);

        assert_eq!(metrics.tool_calls, 3);
        assert_eq!(metrics.errors, 1);
        assert_eq!(metrics.tool_stats.get("Read").unwrap().calls, 2);
        assert_eq!(metrics.tool_stats.get("Read").unwrap().total_time_ms, 80);
        assert_eq!(metrics.tool_stats.get("Bash").unwrap().errors, 1);
    }

    #[test]
    fn test_agent_metrics_avg_time() {
        let mut metrics = AgentMetrics::default();
        assert_eq!(metrics.avg_tool_time_ms(), 0.0);

        metrics.record_tool("Read", 100, false);
        metrics.record_tool("Write", 200, false);
        assert!((metrics.avg_tool_time_ms() - 150.0).abs() < 0.1);
    }
}
