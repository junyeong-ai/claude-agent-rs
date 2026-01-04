//! Core types for the Claude Agent SDK.

pub mod citations;
pub mod content;
pub mod document;
mod message;
pub mod models;
mod response;
pub mod search;
mod tool;

pub use citations::{
    CharLocationCitation, Citation, CitationsConfig, ContentBlockLocationCitation,
    PageLocationCitation, SearchResultLocationCitation,
};
pub use content::{
    ContentBlock, ImageSource, ServerToolUseBlock, TextBlock, ThinkingBlock, ToolResultBlock,
    ToolResultContent, ToolResultContentBlock, ToolUseBlock, WebFetchResultItem,
    WebFetchToolResultBlock, WebFetchToolResultContent, WebFetchToolResultError,
    WebSearchResultItem, WebSearchToolResultBlock, WebSearchToolResultContent,
    WebSearchToolResultError,
};
pub use document::{DocumentBlock, DocumentContentBlock, DocumentSource};
pub use message::{CacheControl, CacheTtl, CacheType, Message, Role, SystemBlock, SystemPrompt};
pub use models::{DEFAULT_COMPACT_THRESHOLD, context_window};
pub use response::{
    ApiResponse, CompactResult, ContentDelta, MessageDeltaData, MessageStartData, ModelUsage,
    PermissionDenial, ServerToolUse, ServerToolUseUsage, StopReason, StreamError, StreamEvent,
    TokenUsage, Usage,
};
pub use search::{SearchResultBlock, SearchResultContentBlock};
pub use tool::{
    ServerTool, ToolDefinition, ToolError, ToolInput, ToolOutput, ToolOutputBlock, ToolResult,
    UserLocation, WebFetchTool, WebSearchTool,
};
