//! Tool registry and trait definitions.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::agent::ToolAccess;
use crate::types::ToolDefinition;

/// Result of a tool execution
#[derive(Debug, Clone)]
pub enum ToolResult {
    /// Successful result with content
    Success(String),
    /// Error result
    Error(String),
    /// Empty success (no content)
    Empty,
}

impl ToolResult {
    /// Create a success result
    pub fn success(content: impl Into<String>) -> Self {
        Self::Success(content.into())
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }

    /// Create an empty result
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Trait for tool implementations
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the JSON schema for input parameters
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with given input
    async fn execute(&self, input: serde_json::Value) -> ToolResult;

    /// Get the tool definition for API
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

/// Registry of available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create an empty registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Create a registry with default tools
    pub fn default_tools(access: &ToolAccess, working_dir: Option<PathBuf>) -> Self {
        let mut registry = Self::new();
        let wd = working_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let all_tools: Vec<Arc<dyn Tool>> = vec![
            // File tools
            Arc::new(super::ReadTool::new(wd.clone())),
            Arc::new(super::WriteTool::new(wd.clone())),
            Arc::new(super::EditTool::new(wd.clone())),
            Arc::new(super::GlobTool::new(wd.clone())),
            Arc::new(super::GrepTool::new(wd.clone())),
            // Shell tools
            Arc::new(super::BashTool::new(wd.clone())),
            // Productivity tools
            Arc::new(super::TodoWriteTool::new()),
            // Web tools
            Arc::new(super::WebSearchTool::new()),
            Arc::new(super::WebFetchTool::new()),
            // Notebook tools
            Arc::new(super::NotebookEditTool::new(wd.clone())),
            // Skill tool for progressive disclosure
            Arc::new(crate::skills::SkillTool::with_defaults()),
        ];

        for tool in all_tools {
            if access.is_allowed(tool.name()) {
                registry.register(tool);
            }
        }

        registry
    }

    /// Create a registry with default tools plus custom skill executor
    pub fn with_skills(
        access: &ToolAccess,
        working_dir: Option<PathBuf>,
        skill_executor: crate::skills::SkillExecutor,
    ) -> Self {
        let mut registry = Self::default_tools(access, working_dir);

        // Replace the default SkillTool with custom executor
        let skill_tool = Arc::new(crate::skills::SkillTool::new(skill_executor));
        if access.is_allowed("Skill") {
            registry.register(skill_tool);
        }

        registry
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    /// Execute a tool by name
    pub async fn execute(&self, name: &str, input: serde_json::Value) -> ToolResult {
        match self.tools.get(name) {
            Some(tool) => tool.execute(input).await,
            None => ToolResult::error(format!("Unknown tool: {}", name)),
        }
    }

    /// Get all tool definitions
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Get tool names
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a tool exists
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result() {
        assert!(!ToolResult::success("ok").is_error());
        assert!(ToolResult::error("fail").is_error());
        assert!(!ToolResult::empty().is_error());
    }
}
