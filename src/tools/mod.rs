//! Built-in tools for the agent.

mod bash;
mod edit;
mod glob;
mod grep;
mod kill;
pub mod notebook;
mod read;
mod registry;
mod todo;
pub mod web;
mod write;

pub use bash::BashTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use kill::KillShellTool;
pub use notebook::NotebookEditTool;
pub use read::ReadTool;
pub use registry::{Tool, ToolRegistry, ToolResult};
pub use todo::TodoWriteTool;
pub use web::{WebFetchTool, WebSearchTool};
pub use write::WriteTool;

pub use crate::agent::ToolAccess;
