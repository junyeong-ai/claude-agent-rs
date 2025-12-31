//! Static Context and Prompt Caching Support
//!
//! StaticContext contains content that is cached for the entire session,
//! leveraging Anthropic's Prompt Caching feature for token efficiency.

use serde::{Deserialize, Serialize};
use crate::types::ToolDefinition;

/// Cache control directive for Anthropic's Prompt Caching
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheControl {
    /// Ephemeral cache (5 minute TTL)
    Ephemeral,
}

/// A system message block with optional cache control
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemBlock {
    /// Block type (always "text")
    #[serde(rename = "type")]
    pub block_type: String,

    /// Text content
    pub text: String,

    /// Cache control directive
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl SystemBlock {
    /// Create a new system block with caching enabled
    pub fn cached(text: impl Into<String>) -> Self {
        Self {
            block_type: "text".to_string(),
            text: text.into(),
            cache_control: Some(CacheControl::Ephemeral),
        }
    }

    /// Create a new system block without caching
    pub fn uncached(text: impl Into<String>) -> Self {
        Self {
            block_type: "text".to_string(),
            text: text.into(),
            cache_control: None,
        }
    }
}

/// Static context that is always loaded (Layer 1)
///
/// This content is cached using Anthropic's Prompt Caching feature
/// for token efficiency across multiple turns.
#[derive(Clone, Debug, Default)]
pub struct StaticContext {
    /// System prompt (base instructions)
    pub system_prompt: String,

    /// CLAUDE.md content (project context)
    pub claude_md: String,

    /// Skill index summary (for routing)
    pub skill_index_summary: String,

    /// Tool definitions
    pub tool_definitions: Vec<ToolDefinition>,

    /// MCP tool metadata
    pub mcp_tool_metadata: Vec<McpToolMeta>,
}

/// Minimal MCP tool metadata for context
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpToolMeta {
    /// Server name
    pub server: String,
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
}

impl StaticContext {
    /// Create a new empty static context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the system prompt
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    /// Set CLAUDE.md content
    pub fn with_claude_md(mut self, content: impl Into<String>) -> Self {
        self.claude_md = content.into();
        self
    }

    /// Set skill index summary
    pub fn with_skill_summary(mut self, summary: impl Into<String>) -> Self {
        self.skill_index_summary = summary.into();
        self
    }

    /// Add tool definitions
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tool_definitions = tools;
        self
    }

    /// Add MCP tool metadata
    pub fn with_mcp_tools(mut self, tools: Vec<McpToolMeta>) -> Self {
        self.mcp_tool_metadata = tools;
        self
    }

    /// Convert to system blocks for API request
    ///
    /// Returns cached system blocks in the correct order.
    pub fn to_system_blocks(&self) -> Vec<SystemBlock> {
        let mut blocks = Vec::new();

        // System prompt (always first, cached)
        if !self.system_prompt.is_empty() {
            blocks.push(SystemBlock::cached(&self.system_prompt));
        }

        // CLAUDE.md (cached)
        if !self.claude_md.is_empty() {
            blocks.push(SystemBlock::cached(&self.claude_md));
        }

        // Skill index summary (cached)
        if !self.skill_index_summary.is_empty() {
            blocks.push(SystemBlock::cached(&self.skill_index_summary));
        }

        // MCP tool summary (cached if present)
        if !self.mcp_tool_metadata.is_empty() {
            let mcp_summary = self.format_mcp_summary();
            blocks.push(SystemBlock::cached(mcp_summary));
        }

        blocks
    }

    /// Format MCP tools as a summary string
    fn format_mcp_summary(&self) -> String {
        let mut lines = vec!["# MCP Server Tools".to_string()];
        for tool in &self.mcp_tool_metadata {
            lines.push(format!(
                "- mcp__{}__{}:  {}",
                tool.server, tool.name, tool.description
            ));
        }
        lines.join("\n")
    }

    /// Compute a hash for cache key purposes
    pub fn content_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.system_prompt.hash(&mut hasher);
        self.claude_md.hash(&mut hasher);
        self.skill_index_summary.hash(&mut hasher);

        for tool in &self.tool_definitions {
            tool.name.hash(&mut hasher);
        }

        for mcp in &self.mcp_tool_metadata {
            mcp.server.hash(&mut hasher);
            mcp.name.hash(&mut hasher);
        }

        format!("{:016x}", hasher.finish())
    }

    /// Estimate token count (rough approximation)
    pub fn estimate_tokens(&self) -> u64 {
        let total_chars = self.system_prompt.len()
            + self.claude_md.len()
            + self.skill_index_summary.len()
            + self
                .mcp_tool_metadata
                .iter()
                .map(|t| t.description.len())
                .sum::<usize>();

        // Rough estimate: ~4 chars per token
        (total_chars / 4) as u64
    }
}

/// Part of static context from a single source
#[derive(Clone, Debug)]
pub struct StaticContextPart {
    /// Content text
    pub content: String,

    /// Priority (higher = loaded first)
    pub priority: i32,

    /// Whether this can be cached
    pub cacheable: bool,
}

impl StaticContextPart {
    /// Create a new cacheable context part
    pub fn cacheable(content: impl Into<String>, priority: i32) -> Self {
        Self {
            content: content.into(),
            priority,
            cacheable: true,
        }
    }

    /// Create a new non-cacheable context part
    pub fn dynamic(content: impl Into<String>, priority: i32) -> Self {
        Self {
            content: content.into(),
            priority,
            cacheable: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_block_cached() {
        let block = SystemBlock::cached("Hello");
        assert_eq!(block.cache_control, Some(CacheControl::Ephemeral));
        assert_eq!(block.block_type, "text");
    }

    #[test]
    fn test_static_context_blocks() {
        let ctx = StaticContext::new()
            .with_system_prompt("You are a helpful assistant")
            .with_claude_md("# Project\nThis is a Rust project");

        let blocks = ctx.to_system_blocks();
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].text.contains("helpful assistant"));
        assert!(blocks[1].text.contains("Rust project"));
    }

    #[test]
    fn test_content_hash_consistency() {
        let ctx1 = StaticContext::new()
            .with_system_prompt("Same prompt")
            .with_claude_md("Same content");

        let ctx2 = StaticContext::new()
            .with_system_prompt("Same prompt")
            .with_claude_md("Same content");

        assert_eq!(ctx1.content_hash(), ctx2.content_hash());
    }

    #[test]
    fn test_content_hash_different() {
        let ctx1 = StaticContext::new().with_system_prompt("Prompt A");
        let ctx2 = StaticContext::new().with_system_prompt("Prompt B");

        assert_ne!(ctx1.content_hash(), ctx2.content_hash());
    }
}
