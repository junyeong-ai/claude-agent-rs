//! # claude-agent
//!
//! Rust SDK for building AI agents with Anthropic's Claude.
//!
//! This crate provides a production-ready, memory-efficient way to build AI agents
//! using the Anthropic Messages API directly, without CLI subprocess dependencies.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use claude_agent::query;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), claude_agent::Error> {
//!     let response = query("What is 2 + 2?").await?;
//!     println!("{}", response);
//!     Ok(())
//! }
//! ```
//!
//! ## Full Agent Example
//!
//! ```rust,no_run
//! use claude_agent::{Agent, AgentEvent, ToolAccess};
//! use futures::StreamExt;
//! use std::pin::pin;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), claude_agent::Error> {
//!     let agent = Agent::builder()
//!         .model("claude-sonnet-4-5")
//!         .tools(ToolAccess::all())
//!         .working_dir("./project")
//!         .build()
//!         .await?;
//!
//!     let stream = agent.execute_stream("Fix the bug").await?;
//!     let mut stream = pin!(stream);
//!     while let Some(event) = stream.next().await {
//!         match event? {
//!             AgentEvent::Text(text) => print!("{}", text),
//!             AgentEvent::Complete(result) => {
//!                 println!("Done: {} tokens", result.total_tokens());
//!             }
//!             _ => {}
//!         }
//!     }
//!     Ok(())
//! }
//! ```

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod agent;
pub mod auth;
pub mod client;
pub mod config;
pub mod context;
pub mod extension;
pub mod hooks;
pub mod mcp;
pub mod permissions;
pub mod prompts;
pub mod session;
pub mod skills;
pub mod tools;
pub mod types;

// Re-exports for convenience
pub use agent::{
    Agent, AgentBuilder, AgentDefinition, AgentEvent, AgentMetrics, AgentOptions, AgentResult,
    AgentState, SubagentType, ToolStats,
};
pub use auth::{
    ApiKeyStrategy, AuthStrategy, BedrockStrategy, ChainProvider, ClaudeCliProvider, Credential,
    CredentialProvider, EnvironmentProvider, ExplicitProvider, FoundryStrategy, OAuthConfig,
    OAuthConfigBuilder, OAuthStrategy, VertexStrategy,
};
pub use client::{Client, ClientBuilder, CloudProvider, Config};
pub use context::{
    ChainMemoryProvider, ContextBuilder, ContextOrchestrator, FileMemoryProvider,
    HttpMemoryProvider, InMemoryProvider, MAX_IMPORT_DEPTH, MemoryContent, MemoryLoader,
    MemoryProvider, RoutingStrategy, RuleIndex, SkillIndex, StaticContext,
};
pub use extension::{Extension, ExtensionContext, ExtensionMeta, ExtensionRegistry};
pub use hooks::{Hook, HookContext, HookEvent, HookInput, HookManager, HookOutput};
pub use permissions::{PermissionDecision, PermissionMode, PermissionPolicy, PermissionResult};
pub use session::{
    CompactExecutor, CompactStrategy, Session, SessionConfig, SessionId, SessionManager,
};
pub use skills::{
    ChainSkillProvider, CommandLoader, FileSkillProvider, InMemorySkillProvider, SkillDefinition,
    SkillExecutor, SkillProvider, SkillRegistry, SkillResult, SkillSourceType, SkillTool,
    SlashCommand,
};
pub use tools::{Tool, ToolAccess, ToolRegistry, ToolResult};
pub use types::{CompactResult, ContentBlock, Message, Role, UserLocation, WebSearchTool};

/// Error type for claude-agent operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// API request failed
    #[error("API error: {message}")]
    Api {
        /// Error message from API
        message: String,
        /// HTTP status code if available
        status: Option<u16>,
    },

    /// Authentication error
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Network or connection error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Tool execution error
    #[error("Tool error: {tool} - {message}")]
    Tool {
        /// Tool name
        tool: String,
        /// Error message
        message: String,
    },

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// IO error (file operations)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Rate limit exceeded
    #[error("Rate limit exceeded, retry after {retry_after:?}")]
    RateLimit {
        /// Suggested retry delay
        retry_after: Option<std::time::Duration>,
    },

    /// Context window exceeded
    #[error("Context window exceeded: {current} / {max} tokens")]
    ContextOverflow {
        /// Current token count
        current: usize,
        /// Maximum allowed tokens
        max: usize,
    },

    /// Execution timed out
    #[error("Execution timed out after {0:?}")]
    Timeout(std::time::Duration),
}

/// Result type alias for claude-agent operations
pub type Result<T> = std::result::Result<T, Error>;

/// Simple query function for one-shot requests
///
/// # Example
///
/// ```rust,no_run
/// # use claude_agent::query;
/// # #[tokio::main]
/// # async fn main() -> Result<(), claude_agent::Error> {
/// let response = query("What is the meaning of life?").await?;
/// println!("{}", response);
/// # Ok(())
/// # }
/// ```
pub async fn query(prompt: &str) -> Result<String> {
    let client = Client::from_env_async().await?;
    client.query(prompt).await
}

/// Stream a response for one-shot requests.
///
/// # Example
///
/// ```rust,no_run
/// # use claude_agent::stream;
/// # use futures::StreamExt;
/// # use std::pin::pin;
/// # #[tokio::main]
/// # async fn main() -> Result<(), claude_agent::Error> {
/// let stream = stream("Tell me a story").await?;
/// let mut stream = pin!(stream);
/// while let Some(chunk) = stream.next().await {
///     print!("{}", chunk?);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn stream(
    prompt: &str,
) -> Result<impl futures::Stream<Item = Result<String>> + Send + 'static + use<>> {
    let client = Client::from_env_async().await?;
    client.stream(prompt).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::Api {
            message: "Invalid API key".to_string(),
            status: Some(401),
        };
        assert!(err.to_string().contains("Invalid API key"));
    }
}
