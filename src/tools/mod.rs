//! Built-in tools for the agent.

mod bash;
mod edit;
mod glob;
mod grep;
mod kill;
pub mod notebook;
mod process;
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
pub use process::{ProcessId, ProcessInfo, ProcessManager};
pub use read::ReadTool;
pub use registry::{Tool, ToolRegistry, ToolResult};

pub(crate) use registry::TypedTool;
pub use todo::TodoWriteTool;
pub use web::WebFetchTool;
pub use write::WriteTool;

pub use crate::agent::ToolAccess;

use std::path::{Path, PathBuf};

pub(crate) fn resolve_path(working_dir: &Path, input_path: &str) -> PathBuf {
    if input_path.starts_with('/') {
        PathBuf::from(input_path)
    } else {
        working_dir.join(input_path)
    }
}
