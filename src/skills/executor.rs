//! Skill executor - runs skills and manages execution context.

use std::sync::Arc;
use std::time::Duration;

use super::{SkillDefinition, SkillRegistry, SkillResult};

const DEFAULT_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

pub type SkillExecutionCallback = Arc<
    dyn Fn(
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
>;

pub struct SkillExecutor {
    registry: SkillRegistry,
    execution_callback: Option<SkillExecutionCallback>,
    callback_timeout: Duration,
    mode: ExecutionMode,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ExecutionMode {
    #[default]
    InlinePrompt,
    Callback,
    DryRun,
}

impl SkillExecutor {
    pub fn new(registry: SkillRegistry) -> Self {
        Self {
            registry,
            execution_callback: None,
            callback_timeout: DEFAULT_CALLBACK_TIMEOUT,
            mode: ExecutionMode::InlinePrompt,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(SkillRegistry::new())
    }

    pub fn with_callback(mut self, callback: SkillExecutionCallback) -> Self {
        self.execution_callback = Some(callback);
        self.mode = ExecutionMode::Callback;
        self
    }

    pub fn with_callback_timeout(mut self, timeout: Duration) -> Self {
        self.callback_timeout = timeout;
        self
    }

    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn registry(&self) -> &SkillRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut SkillRegistry {
        &mut self.registry
    }

    pub async fn execute(&self, name: &str, args: Option<&str>) -> SkillResult {
        let skill = match self.registry.get(name) {
            Some(s) => s,
            None => {
                return SkillResult::error(format!("Skill '{}' not found", name));
            }
        };

        self.execute_skill(skill, args).await
    }

    pub async fn execute_by_trigger(&self, input: &str) -> Option<SkillResult> {
        let skill = self.registry.get_by_trigger(input)?;
        let args = self.extract_args(input, skill);
        Some(self.execute_skill(skill, args.as_deref()).await)
    }

    async fn execute_skill(&self, skill: &SkillDefinition, args: Option<&str>) -> SkillResult {
        let content = skill.content_with_resolved_paths();
        let prompt = self.substitute_args(&content, args);

        let base_result = match self.mode {
            ExecutionMode::DryRun => SkillResult::success(format!(
                "[DRY RUN] Skill '{}' prompt:\n\n{}",
                skill.name, prompt
            )),
            ExecutionMode::Callback => {
                if let Some(ref callback) = self.execution_callback {
                    match tokio::time::timeout(self.callback_timeout, callback(prompt)).await {
                        Ok(Ok(result)) => SkillResult::success(result),
                        Ok(Err(e)) => SkillResult::error(e),
                        Err(_) => SkillResult::error(format!(
                            "Skill callback timed out after {:?}",
                            self.callback_timeout
                        )),
                    }
                } else {
                    SkillResult::error("No execution callback configured")
                }
            }
            ExecutionMode::InlinePrompt => SkillResult::success(format!(
                "Execute the following skill instructions:\n\n---\n{}\n---\n\nSkill: {}\nArguments: {}",
                prompt,
                skill.name,
                args.unwrap_or("(none)")
            )),
        };

        base_result
            .with_allowed_tools(skill.allowed_tools.clone())
            .with_model(skill.model.clone())
            .with_base_dir(skill.base_dir.clone())
    }

    fn substitute_args(&self, content: &str, args: Option<&str>) -> String {
        let args_value = args.unwrap_or("");
        content
            .replace("$ARGUMENTS", args_value)
            .replace("${ARGUMENTS}", args_value)
    }

    fn extract_args(&self, input: &str, skill: &SkillDefinition) -> Option<String> {
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

    pub fn list_skills(&self) -> Vec<&str> {
        self.registry.list()
    }

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

        let skill = SkillDefinition::new("test", "Test", "Content").with_trigger("/test");

        let args = executor.extract_args("/test some arguments here", &skill);
        assert_eq!(args, Some("some arguments here".to_string()));

        let no_args = executor.extract_args("/test", &skill);
        assert!(no_args.is_none());
    }
}
