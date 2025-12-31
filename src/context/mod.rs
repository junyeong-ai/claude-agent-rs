//! Context management with progressive disclosure for optimal token usage.

pub mod builder;
pub mod memory_loader;
pub mod orchestrator;
pub mod routing;
pub mod rule_index;
pub mod skill_index;
pub mod static_context;

// Re-exports
pub use crate::types::TokenUsage;
pub use builder::ContextBuilder;
pub use memory_loader::{MemoryContent, MemoryLoader, RuleFile};
pub use orchestrator::{ContextOrchestrator, ContextWindowState, OrchestratorState};
pub use routing::RoutingStrategy;
pub use rule_index::{LoadedRule, RuleIndex, RuleSource};
pub use skill_index::{SkillIndex, SkillScope, SkillSource};
pub use static_context::{CacheControl, StaticContext, StaticContextPart, SystemBlock};

use thiserror::Error;

/// Errors that can occur in context management
#[derive(Error, Debug)]
pub enum ContextError {
    /// Failed to load context from source
    #[error("Source error: {message}")]
    Source {
        /// Error message describing the failure
        message: String,
    },

    /// Token budget exceeded
    #[error("Token budget exceeded: {current} > {limit}")]
    TokenBudgetExceeded {
        /// Current token count
        current: u64,
        /// Maximum allowed tokens
        limit: u64,
    },

    /// Skill not found in registry
    #[error("Skill not found: {name}")]
    SkillNotFound {
        /// Name of the missing skill
        name: String,
    },

    /// Rule not found in registry
    #[error("Rule not found: {name}")]
    RuleNotFound {
        /// Name of the missing rule
        name: String,
    },

    /// Parse error in skill or rule file
    #[error("Parse error: {message}")]
    Parse {
        /// Error message describing the parse failure
        message: String,
    },

    /// IO error reading files
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for context operations
pub type ContextResult<T> = std::result::Result<T, ContextError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_error_display() {
        let err = ContextError::SkillNotFound {
            name: "test-skill".to_string(),
        };
        assert!(err.to_string().contains("test-skill"));
    }

    #[test]
    fn test_token_budget_error() {
        let err = ContextError::TokenBudgetExceeded {
            current: 250_000,
            limit: 200_000,
        };
        assert!(err.to_string().contains("250000"));
        assert!(err.to_string().contains("200000"));
    }
}
