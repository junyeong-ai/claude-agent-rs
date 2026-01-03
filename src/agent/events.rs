//! Agent events and result types.

use super::state::{AgentMetrics, AgentState};
use crate::types::{Message, StopReason, Usage};

/// Events emitted during agent execution.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Text(String),
    ToolStart {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolEnd {
        id: String,
        output: String,
        is_error: bool,
    },
    Thinking(String),
    ContextUpdate {
        used_tokens: u64,
        max_tokens: u64,
    },
    RulesActivated {
        file_path: String,
        rule_names: Vec<String>,
    },
    CompactStarted,
    CompactCompleted {
        previous_tokens: u64,
        current_tokens: u64,
    },
    Complete(Box<AgentResult>),
}

/// Result of agent execution.
#[derive(Debug, Clone)]
pub struct AgentResult {
    pub text: String,
    pub usage: Usage,
    pub tool_calls: usize,
    pub iterations: usize,
    pub stop_reason: StopReason,
    pub state: AgentState,
    pub metrics: AgentMetrics,
    pub session_id: String,
    pub structured_output: Option<serde_json::Value>,
    pub messages: Vec<Message>,
    /// Unique identifier for this result (like CLI's uuid).
    pub uuid: String,
}

impl AgentResult {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn total_tokens(&self) -> u32 {
        self.usage.total()
    }

    #[must_use]
    pub fn metrics(&self) -> &AgentMetrics {
        &self.metrics
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn extract<T: serde::de::DeserializeOwned>(&self) -> crate::Result<T> {
        let value = self
            .structured_output
            .as_ref()
            .ok_or_else(|| crate::Error::Parse("No structured output available".to_string()))?;
        serde_json::from_value(value.clone()).map_err(|e| crate::Error::Parse(e.to_string()))
    }
}
