//! Context Orchestrator - Progressive Disclosure Engine
//!
//! Manages three-layer context loading:
//! 1. Static context (always loaded, cached)
//! 2. Context-aware loading (rules based on file path)
//! 3. On-demand loading (explicit skill/rule requests)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::skills::SkillDefinition;
use crate::types::{DEFAULT_COMPACT_THRESHOLD, TokenUsage, context_window};

use super::rule_index::{LoadedRule, RulesEngine};
use super::skill_index::SkillIndex;
use super::static_context::StaticContext;

pub struct PromptOrchestrator {
    static_context: StaticContext,
    skill_indices: Vec<SkillIndex>,
    rules_engine: Arc<RulesEngine>,
    model: String,
    current_input_tokens: u64,
    compact_threshold: f32,
    current_file: Option<PathBuf>,
    active_skills: Arc<RwLock<HashMap<String, SkillDefinition>>>,
    active_rule_names: Arc<RwLock<HashSet<String>>>,
}

impl PromptOrchestrator {
    pub fn new(static_context: StaticContext, model: &str) -> Self {
        Self {
            static_context,
            skill_indices: Vec::new(),
            rules_engine: Arc::new(RulesEngine::new()),
            model: model.to_string(),
            current_input_tokens: 0,
            compact_threshold: DEFAULT_COMPACT_THRESHOLD,
            current_file: None,
            active_skills: Arc::new(RwLock::new(HashMap::new())),
            active_rule_names: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub fn with_rules_engine(mut self, engine: RulesEngine) -> Self {
        self.rules_engine = Arc::new(engine);
        self
    }

    pub fn with_skill_indices(mut self, indices: Vec<SkillIndex>) -> Self {
        self.skill_indices = indices;
        self
    }

    pub fn static_context(&self) -> &StaticContext {
        &self.static_context
    }

    pub fn static_context_mut(&mut self) -> &mut StaticContext {
        &mut self.static_context
    }

    pub fn rules_engine(&self) -> &RulesEngine {
        &self.rules_engine
    }

    pub fn max_tokens(&self) -> u64 {
        context_window::for_model(&self.model)
    }

    pub fn current_input_tokens(&self) -> u64 {
        self.current_input_tokens
    }

    pub fn update_usage(&mut self, usage: &TokenUsage) {
        self.current_input_tokens = usage.input_tokens;
    }

    pub fn needs_compact(&self) -> bool {
        let ratio = self.current_input_tokens as f32 / self.max_tokens() as f32;
        ratio > self.compact_threshold
    }

    pub fn available_tokens(&self) -> u64 {
        let threshold = (self.max_tokens() as f32 * self.compact_threshold) as u64;
        threshold.saturating_sub(self.current_input_tokens)
    }

    pub fn usage_percent(&self) -> f32 {
        (self.current_input_tokens as f32 / self.max_tokens() as f32) * 100.0
    }

    pub fn set_current_file(&mut self, path: impl AsRef<Path>) {
        self.current_file = Some(path.as_ref().to_path_buf());
    }

    pub fn current_file(&self) -> Option<&Path> {
        self.current_file.as_deref()
    }

    pub async fn get_rules_for_current_file(&self) -> Vec<LoadedRule> {
        match &self.current_file {
            Some(path) => self.rules_engine.load_matching(path).await,
            None => Vec::new(),
        }
    }

    pub async fn get_rules_for_path(&self, path: &Path) -> Vec<LoadedRule> {
        self.rules_engine.load_matching(path).await
    }

    pub async fn activate_rules_for_file(&self, path: &Path) -> Vec<LoadedRule> {
        let rules = self.rules_engine.load_matching(path).await;
        let mut active = self.active_rule_names.write().await;
        for rule in &rules {
            active.insert(rule.index.name.clone());
        }
        rules
    }

    pub async fn active_rule_names(&self) -> Vec<String> {
        let active = self.active_rule_names.read().await;
        active.iter().cloned().collect()
    }

    pub async fn build_dynamic_context(&self, file_path: Option<&Path>) -> String {
        let Some(path) = file_path else {
            return String::new();
        };

        let rules = self.rules_engine.load_matching(path).await;
        if rules.is_empty() {
            return String::new();
        }

        let mut parts = Vec::with_capacity(rules.len() + 1);
        parts.push(format!("# Active Rules for {}\n", path.display()));
        for rule in rules {
            parts.push(format!("## {}\n{}", rule.index.name, rule.content));
        }

        parts.join("\n\n")
    }

    pub fn find_skills_by_triggers(&self, input: &str) -> Vec<&SkillIndex> {
        self.skill_indices
            .iter()
            .filter(|s| s.matches_triggers(input))
            .collect()
    }

    pub fn find_skill_by_command(&self, input: &str) -> Option<&SkillIndex> {
        self.skill_indices.iter().find(|s| s.matches_command(input))
    }

    pub async fn activate_skill(&self, skill: SkillDefinition) {
        let mut skills = self.active_skills.write().await;
        skills.insert(skill.name.clone(), skill);
    }

    pub async fn deactivate_skill(&self, name: &str) -> bool {
        let mut skills = self.active_skills.write().await;
        skills.remove(name).is_some()
    }

    pub async fn get_active_skill(&self, name: &str) -> Option<SkillDefinition> {
        let skills = self.active_skills.read().await;
        skills.get(name).cloned()
    }

    pub async fn active_skill_names(&self) -> Vec<String> {
        let skills = self.active_skills.read().await;
        skills.keys().cloned().collect()
    }

    pub fn build_skill_summary(&self) -> String {
        if self.skill_indices.is_empty() {
            return String::new();
        }

        let mut lines = vec!["# Available Skills".to_string()];
        for skill in &self.skill_indices {
            lines.push(skill.to_summary_line());
        }
        lines.join("\n")
    }

    pub fn build_rules_summary(&self) -> String {
        self.rules_engine.build_summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::rule_index::RuleIndex;

    #[test]
    fn test_orchestrator_creation() {
        let static_context = StaticContext::new().with_system_prompt("Hello");
        let orchestrator = PromptOrchestrator::new(static_context, "claude-sonnet-4-5");

        assert_eq!(orchestrator.max_tokens(), 200_000);
    }

    #[test]
    fn test_token_tracking() {
        let static_context = StaticContext::new();
        let mut orchestrator = PromptOrchestrator::new(static_context, "claude-sonnet-4-5");

        orchestrator.update_usage(&TokenUsage {
            input_tokens: 100_000,
            output_tokens: 500,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });

        assert!(!orchestrator.needs_compact());
        assert_eq!(orchestrator.usage_percent(), 50.0);

        orchestrator.update_usage(&TokenUsage {
            input_tokens: 170_000,
            output_tokens: 500,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });

        assert!(orchestrator.needs_compact());
    }

    #[tokio::test]
    async fn test_rules_for_path() {
        let mut engine = RulesEngine::new();
        engine.add_index(
            RuleIndex::new("rust")
                .with_paths(vec!["**/*.rs".into()])
                .with_source(super::super::rule_index::RuleSource::InMemory {
                    content: "Use snake_case".into(),
                }),
        );
        engine.add_index(RuleIndex::new("global").with_source(
            super::super::rule_index::RuleSource::InMemory {
                content: "Be helpful".into(),
            },
        ));

        let static_context = StaticContext::new();
        let orchestrator =
            PromptOrchestrator::new(static_context, "claude-sonnet-4-5").with_rules_engine(engine);

        let rules = orchestrator
            .get_rules_for_path(Path::new("src/lib.rs"))
            .await;
        assert_eq!(rules.len(), 2);

        let rules = orchestrator
            .get_rules_for_path(Path::new("src/lib.ts"))
            .await;
        assert_eq!(rules.len(), 1);
    }

    #[tokio::test]
    async fn test_skill_activation() {
        let static_context = StaticContext::new();
        let orchestrator = PromptOrchestrator::new(static_context, "claude-sonnet-4-5");

        let skill = SkillDefinition::new("test", "A test skill", "Test content");
        orchestrator.activate_skill(skill).await;

        assert!(orchestrator.get_active_skill("test").await.is_some());
        assert!(orchestrator.deactivate_skill("test").await);
        assert!(orchestrator.get_active_skill("test").await.is_none());
    }
}
