//! Skill executor - runs skills and manages execution context.
//!
//! The executor handles the actual execution of skills, including
//! argument substitution and context management.

use std::sync::Arc;

use super::{SkillDefinition, SkillRegistry, SkillResult};

/// Callback for executing skill prompts through an LLM
pub type SkillExecutionCallback =
    Arc<dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>> + Send + Sync>;

/// Executor for running skills
pub struct SkillExecutor {
    /// Reference to the skill registry
    registry: SkillRegistry,
    /// Optional execution callback for running skills through LLM
    execution_callback: Option<SkillExecutionCallback>,
    /// Execution mode
    mode: ExecutionMode,
}

/// How skills are executed
#[derive(Clone, Copy, Debug, Default)]
pub enum ExecutionMode {
    /// Return the expanded prompt for the agent to process inline
    #[default]
    InlinePrompt,
    /// Execute through callback and return result
    Callback,
    /// Just return the skill content for inspection
    DryRun,
}

impl SkillExecutor {
    /// Create a new executor with the given registry
    pub fn new(registry: SkillRegistry) -> Self {
        Self {
            registry,
            execution_callback: None,
            mode: ExecutionMode::InlinePrompt,
        }
    }

    /// Create an executor with default skills
    pub fn with_defaults() -> Self {
        Self::new(SkillRegistry::with_defaults())
    }

    /// Set execution callback for running skills through LLM
    pub fn with_callback(mut self, callback: SkillExecutionCallback) -> Self {
        self.execution_callback = Some(callback);
        self.mode = ExecutionMode::Callback;
        self
    }

    /// Set execution mode
    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Get the registry
    pub fn registry(&self) -> &SkillRegistry {
        &self.registry
    }

    /// Get mutable registry access
    pub fn registry_mut(&mut self) -> &mut SkillRegistry {
        &mut self.registry
    }

    /// Execute a skill by name
    pub async fn execute(&self, name: &str, args: Option<&str>) -> SkillResult {
        let skill = match self.registry.get(name) {
            Some(s) => s,
            None => {
                return SkillResult::error(format!("Skill '{}' not found", name));
            }
        };

        self.execute_skill(skill, args).await
    }

    /// Execute a skill by trigger pattern
    pub async fn execute_by_trigger(&self, input: &str) -> Option<SkillResult> {
        let skill = self.registry.get_by_trigger(input)?;
        let args = self.extract_args(input, skill);
        Some(self.execute_skill(skill, args.as_deref()).await)
    }

    /// Execute a skill definition
    async fn execute_skill(&self, skill: &SkillDefinition, args: Option<&str>) -> SkillResult {
        let prompt = self.substitute_args(&skill.content, args);

        let base_result = match self.mode {
            ExecutionMode::DryRun => {
                SkillResult::success(format!(
                    "[DRY RUN] Skill '{}' prompt:\n\n{}",
                    skill.name, prompt
                ))
            }
            ExecutionMode::Callback => {
                if let Some(ref callback) = self.execution_callback {
                    match callback(prompt).await {
                        Ok(result) => SkillResult::success(result),
                        Err(e) => SkillResult::error(e),
                    }
                } else {
                    SkillResult::error("No execution callback configured")
                }
            }
            ExecutionMode::InlinePrompt => {
                // Return the expanded prompt for the agent to execute inline
                // This is the progressive disclosure pattern - the skill content
                // becomes part of the conversation for Claude to act upon
                SkillResult::success(format!(
                    "Execute the following skill instructions:\n\n---\n{}\n---\n\nSkill: {}\nArguments: {}",
                    prompt,
                    skill.name,
                    args.unwrap_or("(none)")
                ))
            }
        };

        // Attach execution context from skill definition
        base_result
            .with_allowed_tools(skill.allowed_tools.clone())
            .with_model(skill.model.clone())
    }

    /// Substitute arguments into skill content
    fn substitute_args(&self, content: &str, args: Option<&str>) -> String {
        let args_value = args.unwrap_or("");

        content
            .replace("$ARGUMENTS", args_value)
            .replace("${ARGUMENTS}", args_value)
    }

    /// Extract arguments from trigger input
    fn extract_args(&self, input: &str, skill: &SkillDefinition) -> Option<String> {
        // Find which trigger matched and extract remaining text
        for trigger in &skill.triggers {
            if let Some(pos) = input.to_lowercase().find(&trigger.to_lowercase()) {
                let after_trigger = &input[pos + trigger.len()..].trim();
                if !after_trigger.is_empty() {
                    return Some(after_trigger.to_string());
                }
            }
        }
        None
    }

    /// List available skills
    pub fn list_skills(&self) -> Vec<&str> {
        self.registry.list()
    }

    /// Check if a skill exists
    pub fn has_skill(&self, name: &str) -> bool {
        self.registry.get(name).is_some()
    }
}

impl Default for SkillExecutor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_args() {
        let executor = SkillExecutor::new(SkillRegistry::new());

        let content = "Do something with $ARGUMENTS and ${ARGUMENTS}";
        let result = executor.substitute_args(content, Some("test args"));

        assert_eq!(result, "Do something with test args and test args");
    }

    #[test]
    fn test_substitute_args_empty() {
        let executor = SkillExecutor::new(SkillRegistry::new());

        let content = "Run with: $ARGUMENTS";
        let result = executor.substitute_args(content, None);

        assert_eq!(result, "Run with: ");
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let executor = SkillExecutor::new(SkillRegistry::new());
        let result = executor.execute("nonexistent", None).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_execute_skill() {
        let mut registry = SkillRegistry::new();
        registry.register(SkillDefinition::new(
            "test-skill",
            "Test",
            "Execute: $ARGUMENTS",
        ));

        let executor = SkillExecutor::new(registry);
        let result = executor.execute("test-skill", Some("my args")).await;

        assert!(result.success);
        assert!(result.output.contains("my args"));
    }

    #[tokio::test]
    async fn test_execute_by_trigger() {
        let mut registry = SkillRegistry::new();
        registry.register(
            SkillDefinition::new("commit", "Commit", "Create commit: $ARGUMENTS")
                .with_trigger("/commit"),
        );

        let executor = SkillExecutor::new(registry);
        let result = executor.execute_by_trigger("/commit fix bug").await;

        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.output.contains("fix bug"));
    }

    #[test]
    fn test_extract_args() {
        let registry = SkillRegistry::new();
        let executor = SkillExecutor::new(registry);

        let skill = SkillDefinition::new("test", "Test", "Content")
            .with_trigger("/test");

        let args = executor.extract_args("/test some arguments here", &skill);
        assert_eq!(args, Some("some arguments here".to_string()));

        let no_args = executor.extract_args("/test", &skill);
        assert!(no_args.is_none());
    }
}
