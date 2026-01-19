//! SkillTool - tool wrapper for skill execution with progressive disclosure.

use std::sync::Arc;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::RwLock;

use super::{SkillExecutor, SkillIndex};
use crate::common::IndexRegistry;
use crate::tools::{ExecutionContext, SchemaTool};
use crate::types::ToolResult;

/// Tool for executing skills with progressive disclosure.
///
/// Skills are stored as lightweight indices (metadata only). Full content
/// is loaded on-demand only when the skill is executed.
pub struct SkillTool {
    executor: Arc<RwLock<SkillExecutor>>,
}

impl SkillTool {
    /// Create a new SkillTool with the given executor.
    pub fn new(executor: SkillExecutor) -> Self {
        Self {
            executor: Arc::new(RwLock::new(executor)),
        }
    }

    /// Create a SkillTool with an empty registry.
    pub fn with_defaults() -> Self {
        Self::new(SkillExecutor::with_defaults())
    }

    /// Create a SkillTool with a pre-populated registry.
    pub fn with_registry(registry: IndexRegistry<SkillIndex>) -> Self {
        Self::new(SkillExecutor::new(registry))
    }

    /// Get a reference to the executor.
    pub fn executor(&self) -> &Arc<RwLock<SkillExecutor>> {
        &self.executor
    }

    /// Generate description with available skills list.
    ///
    /// This method generates a complete description including the dynamic
    /// `<available_skills>` section. Use this when building system prompts
    /// to give the LLM visibility into registered skills.
    ///
    /// The skill list includes only metadata (name, description, tools),
    /// NOT the full content - this is the progressive disclosure pattern.
    pub async fn description_with_skills(&self) -> String {
        let executor = self.executor.read().await;
        let registry = executor.registry();

        let skills_section = if registry.is_empty() {
            "<skill>\n<name>No skills registered</name>\n<description>Register skills using IndexRegistry</description>\n</skill>".to_string()
        } else {
            registry
                .iter()
                .map(|skill| {
                    let tools_hint = if skill.allowed_tools.is_empty() {
                        String::new()
                    } else {
                        format!("\n<tools>{}</tools>", skill.allowed_tools.join(", "))
                    };

                    let args_hint = skill
                        .argument_hint
                        .as_ref()
                        .map(|h| format!("\n<args>{}</args>", h))
                        .unwrap_or_default();

                    format!(
                        "<skill>\n<name>{}</name>\n<description>{}</description>{}{}\n</skill>",
                        skill.name, skill.description, tools_hint, args_hint
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            r#"Execute a skill within the main conversation

<skills_instructions>
When users ask you to perform tasks, check if any of the available skills below can help complete the task more effectively. Skills provide specialized capabilities and domain knowledge.

When users ask you to run a "slash command" or reference "/<something>" (e.g., "/commit", "/review-pr"), they are referring to a skill. Use this tool to invoke the corresponding skill.

<example>
User: "run /commit"
Assistant: [Calls Skill tool with skill: "commit"]
</example>

How to invoke:
- Use this tool with the skill name and optional arguments
- Examples:
  - `skill: "pdf"` - invoke the pdf skill
  - `skill: "commit", args: "-m 'Fix bug'"` - invoke with arguments
  - `skill: "review-pr", args: "123"` - invoke with arguments
  - `skill: "ms-office-suite:pdf"` - invoke using fully qualified name

Important:
- When a skill is relevant, you must invoke this tool IMMEDIATELY as your first action
- NEVER just announce or mention a skill in your text response without actually calling this tool
- This is a BLOCKING REQUIREMENT: invoke the relevant Skill tool BEFORE generating any other response about the task
- Only use skills listed in <available_skills> below
- Do not invoke a skill that is already running
</skills_instructions>

<available_skills>
{}
</available_skills>"#,
            skills_section
        )
    }

    /// Register a skill in the executor's registry.
    pub async fn register_skill(&self, skill: SkillIndex) {
        let mut executor = self.executor.write().await;
        executor.registry_mut().register(skill);
    }

    /// Register multiple skills.
    pub async fn register_skills(&self, skills: impl IntoIterator<Item = SkillIndex>) {
        let mut executor = self.executor.write().await;
        executor.registry_mut().register_all(skills);
    }
}

impl Default for SkillTool {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct SkillInput {
    /// The skill name. E.g., "commit", "review-pr", or "pdf"
    pub skill: String,
    /// Optional arguments for the skill
    #[serde(default)]
    pub args: Option<String>,
}

#[async_trait]
impl SchemaTool for SkillTool {
    type Input = SkillInput;

    const NAME: &'static str = "Skill";
    const DESCRIPTION: &'static str = r#"Execute a skill within the main conversation

<skills_instructions>
When users ask you to perform tasks, check if any of the available skills below can help complete the task more effectively. Skills provide specialized capabilities and domain knowledge.

When users ask you to run a "slash command" or reference "/<something>" (e.g., "/commit", "/review-pr"), they are referring to a skill. Use this tool to invoke the corresponding skill.

<example>
User: "run /commit"
Assistant: [Calls Skill tool with skill: "commit"]
</example>

How to invoke:
- Use this tool with the skill name and optional arguments
- Examples:
  - `skill: "pdf"` - invoke the pdf skill
  - `skill: "commit", args: "-m 'Fix bug'"` - invoke with arguments
  - `skill: "review-pr", args: "123"` - invoke with arguments
  - `skill: "ms-office-suite:pdf"` - invoke using fully qualified name

Important:
- When a skill is relevant, you must invoke this tool IMMEDIATELY as your first action
- NEVER just announce or mention a skill in your text response without actually calling this tool
- This is a BLOCKING REQUIREMENT: invoke the relevant Skill tool BEFORE generating any other response about the task
- Only use skills that are registered and available
- Do not invoke a skill that is already running
</skills_instructions>

Note: For the full list of available skills, use description_with_skills() method when building system prompts."#;

    async fn handle(&self, input: SkillInput, _context: &ExecutionContext) -> ToolResult {
        let executor = self.executor.read().await;
        let result = executor.execute(&input.skill, input.args.as_deref()).await;

        if result.success {
            ToolResult::success(result.output)
        } else {
            ToolResult::error(
                result
                    .error
                    .unwrap_or_else(|| "Skill execution failed".to_string()),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ContentSource;
    use crate::tools::{ExecutionContext, Tool};
    use crate::types::ToolOutput;

    fn test_context() -> ExecutionContext {
        ExecutionContext::default()
    }

    fn test_skill(name: &str, description: &str, content: &str) -> SkillIndex {
        SkillIndex::new(name, description).with_source(ContentSource::in_memory(content))
    }

    #[tokio::test]
    async fn test_skill_tool_execute() {
        let mut registry = IndexRegistry::new();
        registry.register(test_skill("test", "Test skill", "Execute with: $ARGUMENTS"));

        let tool = SkillTool::with_registry(registry);
        let context = test_context();

        let result = tool
            .execute(
                serde_json::json!({
                    "skill": "test",
                    "args": "my args"
                }),
                &context,
            )
            .await;

        assert!(!result.is_error());
        if let ToolOutput::Success(content) = &result.output {
            assert!(content.contains("my args"));
        }
    }

    #[tokio::test]
    async fn test_skill_tool_not_found() {
        let tool = SkillTool::with_defaults();
        let context = test_context();

        let result = tool
            .execute(
                serde_json::json!({
                    "skill": "nonexistent"
                }),
                &context,
            )
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_skill_tool_with_defaults() {
        let tool = SkillTool::with_defaults();
        let executor = tool.executor.read().await;
        assert!(!executor.has_skill("nonexistent"));
    }

    #[tokio::test]
    async fn test_description_with_skills() {
        let mut registry = IndexRegistry::new();
        registry.register(test_skill("test-skill", "A test skill", "template"));
        registry.register(test_skill("another-skill", "Another skill", "template"));

        let tool = SkillTool::with_registry(registry);
        let desc = tool.description_with_skills().await;

        assert!(desc.contains("<skills_instructions>"));
        assert!(desc.contains("</skills_instructions>"));
        assert!(desc.contains("<available_skills>"));
        assert!(desc.contains("</available_skills>"));
        assert!(desc.contains("test-skill"));
        assert!(desc.contains("another-skill"));
    }

    #[tokio::test]
    async fn test_description_with_tools_and_args() {
        let mut registry = IndexRegistry::new();
        registry.register(
            SkillIndex::new("reader", "Read files")
                .with_source(ContentSource::in_memory("content"))
                .with_allowed_tools(["Read", "Grep"])
                .with_argument_hint("<file_path>"),
        );

        let tool = SkillTool::with_registry(registry);
        let desc = tool.description_with_skills().await;

        assert!(desc.contains("<tools>Read, Grep</tools>"));
        assert!(desc.contains("<args><file_path></args>"));
    }

    #[tokio::test]
    async fn test_register_skill() {
        let tool = SkillTool::with_defaults();

        tool.register_skill(test_skill("dynamic", "Dynamic skill", "content"))
            .await;

        let executor = tool.executor.read().await;
        assert!(executor.has_skill("dynamic"));
    }

    #[test]
    fn test_description_has_skills_instructions_tag() {
        use crate::tools::Tool;

        let tool = SkillTool::with_defaults();
        let desc = tool.description();

        assert!(desc.contains("<skills_instructions>"));
        assert!(desc.contains("</skills_instructions>"));
    }
}
