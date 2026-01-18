//! Tool-related types.

mod definition;
mod error;
mod output;
mod reference;
mod server;

pub use definition::ToolDefinition;
pub use error::ToolError;
pub use output::{ToolInput, ToolOutput, ToolOutputBlock, ToolResult};
pub use reference::{
    ToolReference, ToolSearchErrorCode, ToolSearchResult, ToolSearchResultContent,
    ToolSearchToolResult,
};
pub use server::{ServerTool, ToolSearchTool, UserLocation, WebFetchTool, WebSearchTool};
