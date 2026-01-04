//! Agent execution engine.

mod common;
mod config;
mod events;
mod execution;
mod executor;
mod options;
mod request;
mod state;
mod state_formatter;
mod streaming;
mod task;
mod task_output;
mod task_registry;

#[cfg(test)]
mod tests;

pub use config::{
    AgentConfig, AgentModelConfig, BudgetConfig, CacheConfig, ExecutionConfig, PromptConfig,
    SecurityConfig, SystemPromptMode,
};
pub use events::{AgentEvent, AgentResult};
pub use executor::Agent;
pub use options::{AgentBuilder, DEFAULT_COMPACT_KEEP_MESSAGES};
pub use state::{AgentMetrics, AgentState, ToolStats};
pub use task::{TaskInput, TaskOutput, TaskTool};
pub use task_output::{TaskOutputInput, TaskOutputResult, TaskOutputTool, TaskStatus};
pub use task_registry::TaskRegistry;
