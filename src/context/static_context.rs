//! Static Context for Prompt Caching
//!
//! Content that is always loaded and cached for the entire session.

use crate::mcp::make_mcp_name;
use crate::types::{SystemBlock, ToolDefinition};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct StaticContext {
    pub system_prompt: String,
    pub claude_md: String,
    pub skill_index_summary: String,
    pub rules_summary: String,
    pub tool_definitions: Vec<ToolDefinition>,
    pub mcp_tool_metadata: Vec<McpToolMeta>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpToolMeta {
    pub server: String,
    pub name: String,
    pub description: String,
}

impl StaticContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = prompt.into();
        self
    }

    pub fn with_claude_md(mut self, content: impl Into<String>) -> Self {
        self.claude_md = content.into();
        self
    }

    pub fn with_skill_summary(mut self, summary: impl Into<String>) -> Self {
        self.skill_index_summary = summary.into();
        self
    }

    pub fn with_rules_summary(mut self, summary: impl Into<String>) -> Self {
        self.rules_summary = summary.into();
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tool_definitions = tools;
        self
    }

    pub fn with_mcp_tools(mut self, tools: Vec<McpToolMeta>) -> Self {
        self.mcp_tool_metadata = tools;
        self
    }

    pub fn to_system_blocks(&self) -> Vec<SystemBlock> {
        let mut blocks = Vec::new();

        if !self.system_prompt.is_empty() {
            blocks.push(SystemBlock::cached(&self.system_prompt));
        }

        if !self.claude_md.is_empty() {
            blocks.push(SystemBlock::cached(&self.claude_md));
        }

        if !self.skill_index_summary.is_empty() {
            blocks.push(SystemBlock::cached(&self.skill_index_summary));
        }

        if !self.rules_summary.is_empty() {
            blocks.push(SystemBlock::cached(&self.rules_summary));
        }

        if !self.mcp_tool_metadata.is_empty() {
            blocks.push(SystemBlock::cached(self.format_mcp_summary()));
        }

        blocks
    }

    fn format_mcp_summary(&self) -> String {
        let mut lines = vec!["# MCP Server Tools".to_string()];
        for tool in &self.mcp_tool_metadata {
            lines.push(format!(
                "- {}:  {}",
                make_mcp_name(&tool.server, &tool.name),
                tool.description
            ));
        }
        lines.join("\n")
    }

    pub fn content_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.system_prompt.hash(&mut hasher);
        self.claude_md.hash(&mut hasher);
        self.skill_index_summary.hash(&mut hasher);
        self.rules_summary.hash(&mut hasher);

        for tool in &self.tool_definitions {
            tool.name.hash(&mut hasher);
        }

        for mcp in &self.mcp_tool_metadata {
            mcp.server.hash(&mut hasher);
            mcp.name.hash(&mut hasher);
        }

        format!("{:016x}", hasher.finish())
    }

    pub fn estimate_tokens(&self) -> u64 {
        let total_chars = self.system_prompt.len()
            + self.claude_md.len()
            + self.skill_index_summary.len()
            + self.rules_summary.len()
            + self
                .mcp_tool_metadata
                .iter()
                .map(|t| t.description.len())
                .sum::<usize>();

        (total_chars / 4) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CacheType;

    #[test]
    fn test_system_block_cached() {
        let block = SystemBlock::cached("Hello");
        assert!(block.cache_control.is_some());
        let cache_ctrl = block.cache_control.unwrap();
        assert_eq!(cache_ctrl.cache_type, CacheType::Ephemeral);
        assert_eq!(block.block_type, "text");
    }

    #[test]
    fn test_static_context_blocks() {
        let static_context = StaticContext::new()
            .with_system_prompt("You are a helpful assistant")
            .with_claude_md("# Project\nThis is a Rust project");

        let blocks = static_context.to_system_blocks();
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
