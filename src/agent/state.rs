//! Agent state management and definitions.
//!
//! This module defines the core types for agent orchestration including
//! agent definitions, subagent types, and state management.

use serde::{Deserialize, Serialize};

/// Type of subagent that can be spawned
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubagentType {
    /// General-purpose agent for complex, multi-step tasks
    #[default]
    GeneralPurpose,
    /// Fast agent for exploring codebases
    Explore,
    /// Software architect agent for designing implementation plans
    Plan,
    /// Agent for configuring status line settings
    StatuslineSetup,
    /// Agent for answering questions about Claude Code documentation
    ClaudeCodeGuide,
}

impl SubagentType {
    /// Get the default model for this subagent type
    pub fn default_model(&self) -> &'static str {
        match self {
            Self::GeneralPurpose => "claude-sonnet-4-5-20250929",
            Self::Explore => "claude-haiku-4-5-20251001",
            Self::Plan => "claude-sonnet-4-5-20250929",
            Self::StatuslineSetup => "claude-haiku-3-5-20241022",
            Self::ClaudeCodeGuide => "claude-haiku-3-5-20241022",
        }
    }

    /// Get a description of what this subagent type does
    pub fn description(&self) -> &'static str {
        match self {
            Self::GeneralPurpose => {
                "General-purpose agent for researching complex questions and executing multi-step tasks"
            }
            Self::Explore => {
                "Fast agent specialized for exploring codebases, finding files, and searching code"
            }
            Self::Plan => {
                "Software architect agent for designing implementation plans and strategies"
            }
            Self::StatuslineSetup => "Agent for configuring user's Claude Code status line settings",
            Self::ClaudeCodeGuide => {
                "Agent for answering questions about Claude Code features and documentation"
            }
        }
    }
}


/// Definition of an agent that can be spawned
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Type of subagent
    pub subagent_type: SubagentType,
    /// Short description of the task (3-5 words)
    pub description: String,
    /// The detailed task prompt
    pub prompt: String,
    /// Optional model override
    pub model: Option<String>,
    /// Whether to run in background
    pub run_in_background: bool,
    /// Optional agent ID to resume from
    pub resume: Option<String>,
}

impl AgentDefinition {
    /// Create a new agent definition
    pub fn new(subagent_type: SubagentType, description: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            subagent_type,
            description: description.into(),
            prompt: prompt.into(),
            model: None,
            run_in_background: false,
            resume: None,
        }
    }

    /// Set the model for this agent
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set to run in background
    pub fn in_background(mut self) -> Self {
        self.run_in_background = true;
        self
    }

    /// Set an agent ID to resume from
    pub fn resume_from(mut self, agent_id: impl Into<String>) -> Self {
        self.resume = Some(agent_id.into());
        self
    }

    /// Get the effective model (custom or default for type)
    pub fn effective_model(&self) -> &str {
        self.model.as_deref().unwrap_or_else(|| self.subagent_type.default_model())
    }
}

/// Current state of an agent execution
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    /// Agent is initializing
    #[default]
    Initializing,
    /// Agent is running and processing
    Running,
    /// Agent is waiting for tool results
    WaitingForToolResults,
    /// Agent is waiting for user input
    WaitingForUserInput,
    /// Agent is in plan mode, awaiting approval
    PlanMode,
    /// Agent has completed successfully
    Completed,
    /// Agent encountered an error and stopped
    Failed,
    /// Agent was cancelled by user
    Cancelled,
}

impl AgentState {
    /// Check if the agent is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Check if the agent is waiting for something
    pub fn is_waiting(&self) -> bool {
        matches!(
            self,
            Self::WaitingForToolResults | Self::WaitingForUserInput | Self::PlanMode
        )
    }

    /// Check if the agent can continue processing
    pub fn can_continue(&self) -> bool {
        matches!(self, Self::Running | Self::Initializing)
    }
}


/// Metrics collected during agent execution
#[derive(Debug, Clone, Default)]
pub struct AgentMetrics {
    /// Total iterations completed
    pub iterations: usize,
    /// Total tool calls made
    pub tool_calls: usize,
    /// Input tokens consumed
    pub input_tokens: u32,
    /// Output tokens generated
    pub output_tokens: u32,
    /// Time spent in execution (milliseconds)
    pub execution_time_ms: u64,
    /// Number of errors encountered (non-fatal)
    pub errors: usize,
}

impl AgentMetrics {
    /// Get total tokens used
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }

    /// Add usage from another source
    pub fn add_usage(&mut self, input: u32, output: u32) {
        self.input_tokens += input;
        self.output_tokens += output;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_type_defaults() {
        assert_eq!(SubagentType::default(), SubagentType::GeneralPurpose);
        assert!(SubagentType::Explore.default_model().contains("haiku"));
        assert!(SubagentType::Plan.default_model().contains("sonnet"));
    }

    #[test]
    fn test_agent_definition() {
        let def = AgentDefinition::new(
            SubagentType::Explore,
            "Search codebase",
            "Find all files containing 'error'",
        )
        .with_model("claude-opus-4-5-20251101")
        .in_background();

        assert_eq!(def.subagent_type, SubagentType::Explore);
        assert!(def.run_in_background);
        assert_eq!(def.effective_model(), "claude-opus-4-5-20251101");
    }

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
}
