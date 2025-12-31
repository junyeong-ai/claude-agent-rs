//! Agent execution engine.
//!
//! This module provides the core agent loop that executes tools and manages
//! conversation context.

mod context;
mod executor;
mod interaction;
mod options;
mod plan;
mod state;
mod task;
mod task_output;

pub use context::ConversationContext;
pub use executor::{Agent, AgentEvent, AgentResult};
pub use interaction::{AskUserQuestionTool, Question, QuestionOption};
pub use options::{AgentBuilder, AgentOptions, ToolAccess};
pub use plan::{EnterPlanModeTool, ExitPlanModeTool, PlanModeState};
pub use state::{AgentDefinition, AgentMetrics, AgentState, SubagentType};
pub use task::{TaskInput, TaskOutput, TaskTool};
pub use task_output::{TaskOutputInput, TaskOutputResult, TaskOutputTool, TaskStatus};
