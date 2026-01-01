//! Core types for the Claude Agent SDK.

mod content;
mod message;
pub mod models;
mod response;
mod tool;

pub use content::{ContentBlock, ImageSource, TextBlock, ToolResultBlock, ToolUseBlock};
pub use message::{CacheControl, Message, Role, SystemBlock, SystemPrompt};
pub use models::{context_window, DEFAULT_COMPACT_THRESHOLD};
pub use response::{
    ApiResponse, CompactResult, ContentDelta, MessageDeltaData, MessageStartData, StopReason,
    StreamError, StreamEvent, TokenUsage, Usage,
};
pub use tool::{ToolDefinition, ToolInput, ToolOutput, UserLocation, WebSearchTool};
