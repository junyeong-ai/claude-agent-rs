//! Context Orchestrator - Token Budget and Context Loading Management

use std::collections::HashMap;
use std::path::Path;

use crate::skills::SkillDefinition;
use crate::types::{DEFAULT_COMPACT_THRESHOLD, TokenUsage, context_window};

use super::rule_index::{LoadedRule, RulesEngine};
use super::skill_index::SkillIndex;
use super::static_context::StaticContext;

/// Context window state tracking
#[derive(Clone, Debug)]
pub struct ContextWindowState {
    /// Maximum tokens for the model
    pub max_tokens: u64,
    /// Current accumulated input tokens (from API usage)
    pub current_input_tokens: u64,
    /// Compact threshold (e.g., 0.8 = 80%)
    pub compact_threshold: f32,
}

impl ContextWindowState {
    /// Create state for a specific model
    pub fn for_model(model: &str) -> Self {
        Self {
            max_tokens: context_window::for_model(model),
            current_input_tokens: 0,
            compact_threshold: DEFAULT_COMPACT_THRESHOLD,
        }
    }

    /// Check if compact is needed
    pub fn needs_compact(&self) -> bool {
        let usage_ratio = self.current_input_tokens as f32 / self.max_tokens as f32;
        usage_ratio > self.compact_threshold
    }

    /// Update from API response usage
    pub fn update_from_usage(&mut self, usage: &TokenUsage) {
        self.current_input_tokens = usage.input_tokens;
    }

    /// Available tokens before compact threshold
    pub fn available_tokens(&self) -> u64 {
        let threshold_tokens = (self.max_tokens as f32 * self.compact_threshold) as u64;
        threshold_tokens.saturating_sub(self.current_input_tokens)
    }

    /// Current usage as a percentage
    pub fn usage_percent(&self) -> f32 {
        (self.current_input_tokens as f32 / self.max_tokens as f32) * 100.0
    }
}

/// Orchestrator state snapshot
#[derive(Clone, Debug)]
pub struct OrchestratorState {
    /// Context window tracking
    pub context_state: ContextWindowState,
    /// Currently active skills
    pub active_skills: HashMap<String, SkillDefinition>,
    /// Currently applied rules
    pub active_rules: HashMap<String, LoadedRule>,
    /// Current working file path
    pub current_file_path: Option<std::path::PathBuf>,
}

/// Context Orchestrator for Progressive Disclosure
///
/// Manages the three-layer context loading:
/// 1. Static context (always loaded, cached)
/// 2. Context-aware loading (rules/skills based on conditions)
/// 3. On-demand loading (explicit tool requests)
pub struct ContextOrchestrator {
    /// Static context (Layer 1)
    static_context: StaticContext,

    /// Skill indices (for routing)
    skill_indices: Vec<SkillIndex>,

    /// Rules engine
    rules_engine: RulesEngine,

    /// Context window state
    context_state: ContextWindowState,

    /// Active skills (loaded on activation)
    active_skills: HashMap<String, SkillDefinition>,

    /// Active rules (loaded on match)
    active_rules: HashMap<String, LoadedRule>,

    /// Current file being worked on
    current_file: Option<std::path::PathBuf>,
}

impl ContextOrchestrator {
    /// Create a new orchestrator with static context
    pub fn new(static_context: StaticContext, model: &str) -> Self {
        Self {
            static_context,
            skill_indices: Vec::new(),
            rules_engine: RulesEngine::new(),
            context_state: ContextWindowState::for_model(model),
            active_skills: HashMap::new(),
            active_rules: HashMap::new(),
            current_file: None,
        }
    }

    /// Add skill indices for routing
    pub fn add_skill_indices(&mut self, indices: Vec<SkillIndex>) {
        self.skill_indices.extend(indices);
    }

    /// Add rule indices for conditional loading
    pub fn add_rule_indices(&mut self, indices: Vec<super::rule_index::RuleIndex>) {
        for index in indices {
            self.rules_engine.add_index(index);
        }
    }

    /// Get static context reference
    pub fn static_context(&self) -> &StaticContext {
        &self.static_context
    }

    /// Get mutable static context reference
    pub fn static_context_mut(&mut self) -> &mut StaticContext {
        &mut self.static_context
    }

    /// Get context window state
    pub fn context_state(&self) -> &ContextWindowState {
        &self.context_state
    }

    /// Update token usage from API response
    pub fn update_usage(&mut self, usage: &TokenUsage) {
        self.context_state.update_from_usage(usage);
    }

    /// Check if compact is needed
    pub fn needs_compact(&self) -> bool {
        self.context_state.needs_compact()
    }

    /// Set the current working file
    pub fn set_current_file(&mut self, path: impl AsRef<Path>) {
        self.current_file = Some(path.as_ref().to_path_buf());
    }

    /// Get active skill names
    pub fn active_skill_names(&self) -> Vec<&str> {
        self.active_skills.keys().map(|s| s.as_str()).collect()
    }

    /// Get active rule names
    pub fn active_rule_names(&self) -> Vec<&str> {
        self.active_rules.keys().map(|s| s.as_str()).collect()
    }

    /// Find matching skill indices by trigger keywords
    pub fn find_skills_by_triggers(&self, input: &str) -> Vec<&SkillIndex> {
        self.skill_indices
            .iter()
            .filter(|s| s.matches_triggers(input))
            .collect()
    }

    /// Find skill by slash command
    pub fn find_skill_by_command(&self, input: &str) -> Option<&SkillIndex> {
        self.skill_indices.iter().find(|s| s.matches_command(input))
    }

    /// Activate a skill by adding it to active skills
    pub fn activate_skill(&mut self, skill: SkillDefinition) {
        self.active_skills.insert(skill.name.clone(), skill);
    }

    /// Deactivate a skill
    pub fn deactivate_skill(&mut self, name: &str) -> bool {
        self.active_skills.remove(name).is_some()
    }

    /// Get an active skill by name
    pub fn get_active_skill(&self, name: &str) -> Option<&SkillDefinition> {
        self.active_skills.get(name)
    }

    /// Activate a rule by adding it to active rules
    pub fn activate_rule(&mut self, rule: LoadedRule) {
        self.active_rules.insert(rule.index.name.clone(), rule);
    }

    /// Get current orchestrator state snapshot
    pub fn state(&self) -> OrchestratorState {
        OrchestratorState {
            context_state: self.context_state.clone(),
            active_skills: self.active_skills.clone(),
            active_rules: self.active_rules.clone(),
            current_file_path: self.current_file.clone(),
        }
    }

    /// Build skill index summary for static context
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

    /// Build rules summary
    pub fn build_rules_summary(&self) -> String {
        self.rules_engine.summary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_input_tokens: 800,
            cache_creation_input_tokens: 0,
        };

        assert_eq!(usage.total(), 1500);
        assert_eq!(usage.cache_hit_rate(), 0.8);
    }

    #[test]
    fn test_context_window_state() {
        let mut state = ContextWindowState::for_model("claude-sonnet-4-5");
        assert_eq!(state.max_tokens, 200_000);

        state.current_input_tokens = 100_000;
        assert!(!state.needs_compact()); // 50% < 80%

        state.current_input_tokens = 170_000;
        assert!(state.needs_compact()); // 85% > 80%
    }

    #[test]
    fn test_orchestrator_creation() {
        let ctx = StaticContext::new().with_system_prompt("Hello");
        let orchestrator = ContextOrchestrator::new(ctx, "claude-sonnet-4-5");

        assert_eq!(orchestrator.context_state().max_tokens, 200_000);
        assert!(orchestrator.active_skill_names().is_empty());
    }
}
