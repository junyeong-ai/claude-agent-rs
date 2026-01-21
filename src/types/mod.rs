//! Core types for the Claude Agent SDK.

pub mod citations;
pub mod content;
pub mod document;
mod message;
mod response;
pub mod search;
mod tool;

pub use crate::models::{DEFAULT_COMPACT_THRESHOLD, context_window};
pub use citations::{
    CharLocationCitation, Citation, CitationsConfig, ContentBlockLocationCitation,
    PageLocationCitation, SearchResultLocationCitation,
};
pub use content::{
    ContentBlock, ImageSource, ServerToolError, ServerToolUseBlock, TextBlock, ThinkingBlock,
    ToolResultBlock, ToolResultContent, ToolResultContentBlock, ToolUseBlock, WebFetchResultItem,
    WebFetchToolResultBlock, WebFetchToolResultContent, WebSearchResultItem,
    WebSearchToolResultBlock, WebSearchToolResultContent,
};
pub use document::{DocumentBlock, DocumentContentBlock, DocumentSource};
pub use message::{CacheControl, CacheTtl, CacheType, Message, Role, SystemBlock, SystemPrompt};
pub use response::{
    ApiResponse, CompactResult, ContentDelta, MessageDeltaData, MessageStartData, ModelUsage,
    PermissionDenial, ServerToolUse, ServerToolUseUsage, StopReason, StreamError, StreamEvent,
    TokenUsage, Usage,
};
pub use search::{SearchResultBlock, SearchResultContentBlock};
pub use tool::{
    ServerTool, ToolDefinition, ToolError, ToolInput, ToolOutput, ToolOutputBlock, ToolReference,
    ToolResult, ToolSearchErrorCode, ToolSearchResult, ToolSearchResultContent, ToolSearchTool,
    ToolSearchToolResult, UserLocation, WebFetchTool, WebSearchTool,
};
