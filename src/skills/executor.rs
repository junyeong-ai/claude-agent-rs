//! Skill executor - runs skills with lazy content loading.

use std::sync::Arc;
use std::time::Duration;

use super::{SkillIndex, SkillResult};
use crate::common::{IndexRegistry, Named};

const DEFAULT_CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

pub type SkillExecutionCallback = Arc<
    dyn Fn(
            String,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>
        + Send
        + Sync,
>;

/// Skill executor using IndexRegistry for progressive disclosure.
///
/// Skills are stored as lightweight indices (metadata only). Full content
/// is loaded on-demand only when the skill is executed.
pub struct SkillExecutor {
    registry: IndexRegistry<SkillIndex>,
    execution_callback: Option<SkillExecutionCallback>,
    callback_timeout: Duration,
    mode: ExecutionMode,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ExecutionMode {
    /// Return skill content as inline prompt (default)
    #[default]
    InlinePrompt,
    /// Execute via callback function
    Callback,
    /// Return what would be executed without running
    DryRun,
}

impl SkillExecutor {
    /// Create a new executor with an IndexRegistry.
    pub fn new(registry: IndexRegistry<SkillIndex>) -> Self {
        Self {
            registry,
            execution_callback: None,
            callback_timeout: DEFAULT_CALLBACK_TIMEOUT,
            mode: ExecutionMode::InlinePrompt,
        }
    }

    /// Create an executor with an empty registry.
    pub fn with_defaults() -> Self {
        Self::new(IndexRegistry::new())
    }

    /// Set execution callback.
    pub fn with_callback(mut self, callback: SkillExecutionCallback) -> Self {
        self.execution_callback = Some(callback);
        self.mode = ExecutionMode::Callback;
        self
    }

    /// Set callback timeout.
    pub fn with_callback_timeout(mut self, timeout: Duration) -> Self {
        self.callback_timeout = timeout;
        self
    }

    /// Set execution mode.
    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Get the skill registry.
    pub fn registry(&self) -> &IndexRegistry<SkillIndex> {
        &self.registry
    }

    /// Get mutable access to the registry.
    pub fn registry_mut(&mut self) -> &mut IndexRegistry<SkillIndex> {
        &mut self.registry
    }

    /// Consume the executor and return the registry.
    pub fn into_registry(self) -> IndexRegistry<SkillIndex> {
        self.registry
    }

    /// Execute a skill by name.
    ///
    /// This triggers lazy loading of the skill content.
    pub async fn execute(&self, name: &str, args: Option<&str>) -> SkillResult {
        let skill = match self.registry.get(name) {
            Some(s) => s.clone(),
            None => {
                return SkillResult::error(format!("Skill '{}' not found", name));
            }
        };

        self.execute_skill(&skill, args).await
    }

    /// Execute by trigger matching.
    pub async fn execute_by_trigger(&self, input: &str) -> Option<SkillResult> {
        // Find matching skill
        let skill = self.registry.iter().find(|s| s.matches_triggers(input))?;
        let skill = skill.clone();

        let args = self.extract_args(input, &skill);
        Some(self.execute_skill(&skill, args.as_deref()).await)
    }

    /// Execute a skill directly.
    async fn execute_skill(&self, skill: &SkillIndex, args: Option<&str>) -> SkillResult {
        // Load content using registry cache (fixes cache bypass bug)
        let content = match self.registry.load_content(skill.name()).await {
            Ok(c) => c,
            Err(e) => {
                return SkillResult::error(format!("Failed to load skill '{}': {}", skill.name, e));
            }
        };

        // Use the new execute() method for full processing pipeline
        let prompt = skill.execute(args.unwrap_or(""), &content).await;

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
            .with_base_dir(skill.base_dir())
    }

    fn extract_args(&self, input: &str, skill: &SkillIndex) -> Option<String> {
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

    /// List all skill names.
    pub fn list_skills(&self) -> Vec<&str> {
        self.registry.list()
    }

    /// Check if a skill exists.
    pub fn has_skill(&self, name: &str) -> bool {
        self.registry.contains(name)
    }

    /// Get a skill by name.
    pub fn get_skill(&self, name: &str) -> Option<&SkillIndex> {
        self.registry.get(name)
    }

    /// Find skill by trigger match.
    pub fn get_by_trigger(&self, input: &str) -> Option<&SkillIndex> {
        self.registry.iter().find(|s| s.matches_triggers(input))
    }

    /// Build summary for system prompt.
    pub fn build_summary(&self) -> String {
        self.registry.build_summary()
    }
}

impl Default for SkillExecutor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use crate::common::ContentSource;

    use super::*;

    fn test_skill(name: &str, content: &str) -> SkillIndex {
        SkillIndex::new(name, format!("Test skill: {}", name))
            .with_source(ContentSource::in_memory(content))
    }

    #[test]
    fn test_substitute_args() {
        let content = "Do something with $ARGUMENTS and ${ARGUMENTS}";
        let result = SkillIndex::substitute_args(content, Some("test args"));
        assert_eq!(result, "Do something with test args and test args");
    }

    #[test]
    fn test_substitute_args_empty() {
        let content = "Run with: $ARGUMENTS";
        let result = SkillIndex::substitute_args(content, None);
        assert_eq!(result, "Run with: ");
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let executor = SkillExecutor::with_defaults();
        let result = executor.execute("nonexistent", None).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_execute_skill() {
        let mut registry = IndexRegistry::new();
        registry.register(test_skill("test-skill", "Execute: $ARGUMENTS"));

        let executor = SkillExecutor::new(registry);
        let result = executor.execute("test-skill", Some("my args")).await;

        assert!(result.success);
        assert!(result.output.contains("my args"));
    }

    #[tokio::test]
    async fn test_execute_by_trigger() {
        let mut registry = IndexRegistry::new();
        registry.register(
            SkillIndex::new("commit", "Create commit")
                .with_source(ContentSource::in_memory("Create commit: $ARGUMENTS"))
                .with_triggers(["/commit"]),
        );

        let executor = SkillExecutor::new(registry);
        let result = executor.execute_by_trigger("/commit fix bug").await;

        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.success);
        assert!(result.output.contains("fix bug"));
    }

    #[tokio::test]
    async fn test_dry_run_mode() {
        let mut registry = IndexRegistry::new();
        registry.register(test_skill("test", "Test content"));

        let executor = SkillExecutor::new(registry).with_mode(ExecutionMode::DryRun);
        let result = executor.execute("test", None).await;

        assert!(result.success);
        assert!(result.output.contains("[DRY RUN]"));
    }

    #[test]
    fn test_list_skills() {
        let mut registry = IndexRegistry::new();
        registry.register(test_skill("a", "A"));
        registry.register(test_skill("b", "B"));

        let executor = SkillExecutor::new(registry);
        let names = executor.list_skills();

        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn test_has_skill() {
        let mut registry = IndexRegistry::new();
        registry.register(test_skill("exists", "Content"));

        let executor = SkillExecutor::new(registry);
        assert!(executor.has_skill("exists"));
        assert!(!executor.has_skill("missing"));
    }

    #[tokio::test]
    async fn test_skill_with_allowed_tools() {
        let mut registry = IndexRegistry::new();
        registry.register(
            SkillIndex::new("reader", "Read files")
                .with_source(ContentSource::in_memory("Read: $ARGUMENTS"))
                .with_allowed_tools(["Read", "Grep"]),
        );

        let executor = SkillExecutor::new(registry);
        let result = executor.execute("reader", None).await;

        assert!(result.success);
        assert_eq!(result.allowed_tools, vec!["Read", "Grep"]);
    }

    #[tokio::test]
    async fn test_skill_with_model() {
        let mut registry = IndexRegistry::new();
        registry.register(
            SkillIndex::new("fast", "Fast task")
                .with_source(ContentSource::in_memory("Do: $ARGUMENTS"))
                .with_model("claude-haiku-4-5-20251001"),
        );

        let executor = SkillExecutor::new(registry);
        let result = executor.execute("fast", None).await;

        assert!(result.success);
        assert_eq!(result.model, Some("claude-haiku-4-5-20251001".to_string()));
    }

    #[test]
    fn test_build_summary() {
        let mut registry = IndexRegistry::new();
        registry.register(SkillIndex::new("commit", "Create commits"));
        registry.register(SkillIndex::new("review", "Review code"));

        let executor = SkillExecutor::new(registry);
        let summary = executor.build_summary();

        assert!(summary.contains("commit"));
        assert!(summary.contains("review"));
    }
}
