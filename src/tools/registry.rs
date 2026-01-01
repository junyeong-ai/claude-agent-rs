//! Tool registry and trait definitions.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::agent::{TaskOutputTool, TaskTool, ToolAccess};
use crate::types::ToolDefinition;

/// Result of a tool execution.
#[derive(Debug, Clone)]
pub enum ToolResult {
    /// Successful result with content.
    Success(String),
    /// Error result.
    Error(String),
    /// Empty success (no content).
    Empty,
}

impl ToolResult {
    /// Create a success result.
    #[must_use]
    pub fn success(content: impl Into<String>) -> Self {
        Self::Success(content.into())
    }

    /// Create an error result.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }

    /// Create an empty result.
    #[must_use]
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Check if this is an error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }
}

/// Base trait for tool implementations (object-safe).
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name.
    fn name(&self) -> &str;

    /// Get the tool description.
    fn description(&self) -> &str;

    /// Get the JSON schema for input parameters.
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with given input.
    async fn execute(&self, input: serde_json::Value) -> ToolResult;

    /// Get the tool definition for API.
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

/// Internal trait for typed tool implementations with automatic schema generation.
///
/// This trait is intentionally `pub(crate)` - external users should use the [`Tool`] trait.
/// The blanket implementation automatically converts any `TypedTool` into a `Tool`.
///
/// # Design Rationale
/// - Follows the Handler pattern (similar to actix-web, axum)
/// - `handle` method processes typed input
/// - Automatic JSON schema generation via `schemars`
/// - Object-safe `Tool` trait exposed for `dyn Tool` usage in registries
#[async_trait]
pub(crate) trait TypedTool: Send + Sync {
    /// Input type with automatic JSON schema derivation.
    type Input: JsonSchema + DeserializeOwned + Send;

    /// Tool name.
    const NAME: &'static str;

    /// Tool description.
    const DESCRIPTION: &'static str;

    /// Handle the tool invocation with typed input.
    async fn handle(&self, input: Self::Input) -> ToolResult;

    /// Generate JSON schema from input type.
    fn input_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(Self::Input);
        serde_json::to_value(schema).unwrap_or_else(|_| serde_json::json!({"type": "object"}))
    }
}

/// Blanket implementation: TypedTool automatically implements Tool.
#[async_trait]
impl<T: TypedTool + 'static> Tool for T {
    fn name(&self) -> &str {
        T::NAME
    }

    fn description(&self) -> &str {
        T::DESCRIPTION
    }

    fn input_schema(&self) -> serde_json::Value {
        T::input_schema()
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        match serde_json::from_value::<T::Input>(input) {
            Ok(typed) => TypedTool::handle(self, typed).await,
            Err(e) => ToolResult::error(format!("Invalid input: {}", e)),
        }
    }
}

/// Registry of available tools
#[derive(Clone)]
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

        // Shared process manager for bash and kill tools
        let process_manager = Arc::new(super::ProcessManager::new());

        let all_tools: Vec<Arc<dyn Tool>> = vec![
            // File tools
            Arc::new(super::ReadTool::new(wd.clone())),
            Arc::new(super::WriteTool::new(wd.clone())),
            Arc::new(super::EditTool::new(wd.clone())),
            Arc::new(super::GlobTool::new(wd.clone())),
            Arc::new(super::GrepTool::new(wd.clone())),
            Arc::new(super::NotebookEditTool::new(wd.clone())),
            // Shell tools (shared ProcessManager)
            Arc::new(super::BashTool::with_process_manager(
                wd.clone(),
                process_manager.clone(),
            )),
            Arc::new(super::KillShellTool::with_process_manager(process_manager)),
            // Web tools
            Arc::new(super::WebFetchTool::new()),
            // Agent tools
            Arc::new(TaskTool::new()),
            Arc::new(TaskOutputTool::new()),
            // Productivity tools
            Arc::new(super::TodoWriteTool::new()),
            // Skill tool
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
