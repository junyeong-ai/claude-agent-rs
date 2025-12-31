//! Routing Strategy for Skill Discovery
//!
//! Determines how skills are discovered and activated based on user input.

use serde::{Deserialize, Serialize};

use super::skill_index::SkillIndex;

/// Strategy for routing user input to skills
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoutingStrategy {
    /// Explicit skill invocation via slash command (/skill-name)
    Explicit {
        /// The matched skill name
        skill_name: String,
    },

    /// Keyword-based matching using trigger words
    KeywordMatch {
        /// Matched trigger keywords
        matched_triggers: Vec<String>,
        /// Candidate skill names
        skill_names: Vec<String>,
    },

    /// Semantic matching (delegated to Claude)
    ///
    /// When no explicit or keyword match is found, Claude uses
    /// the skill index summaries to decide.
    Semantic {
        /// Confidence score (0.0 to 1.0)
        confidence: f32,
    },

    /// No skill routing needed
    NoSkill,
}

impl RoutingStrategy {
    /// Check if this is an explicit invocation
    pub fn is_explicit(&self) -> bool {
        matches!(self, Self::Explicit { .. })
    }

    /// Check if any skill was matched
    pub fn has_skill(&self) -> bool {
        !matches!(self, Self::NoSkill)
    }

    /// Get the primary skill name if available
    pub fn primary_skill(&self) -> Option<&str> {
        match self {
            Self::Explicit { skill_name } => Some(skill_name),
            Self::KeywordMatch { skill_names, .. } => skill_names.first().map(|s| s.as_str()),
            Self::Semantic { .. } => None,
            Self::NoSkill => None,
        }
    }
}

/// Skill discovery mode configuration
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDiscoveryMode {
    /// Only explicit slash commands activate skills
    ExplicitOnly,

    /// Index-based discovery (default)
    ///
    /// Skills are discoverable via:
    /// - Slash commands (/skill-name)
    /// - Trigger keyword matching
    /// - Semantic matching (Claude uses skill descriptions)
    #[default]
    IndexBased,

    /// RAG-assisted discovery (optional feature)
    ///
    /// Uses embeddings for semantic similarity matching
    /// before delegating to Claude.
    RagAssisted {
        /// Embedding model to use
        embedding_model: String,
        /// Minimum similarity threshold
        similarity_threshold: f32,
    },
}

/// Router for skill discovery
#[derive(Debug)]
pub struct SkillRouter {
    /// Discovery mode
    mode: SkillDiscoveryMode,
}

impl SkillRouter {
    /// Create a new router with the specified mode
    pub fn new(mode: SkillDiscoveryMode) -> Self {
        Self { mode }
    }

    /// Route user input to a skill
    pub fn route(&self, input: &str, skill_indices: &[SkillIndex]) -> RoutingStrategy {
        match &self.mode {
            SkillDiscoveryMode::ExplicitOnly => self.route_explicit_only(input, skill_indices),
            SkillDiscoveryMode::IndexBased => self.route_index_based(input, skill_indices),
            SkillDiscoveryMode::RagAssisted { .. } => {
                // RAG would be handled externally; fall back to index-based
                self.route_index_based(input, skill_indices)
            }
        }
    }

    /// Route with explicit-only mode
    fn route_explicit_only(&self, input: &str, skill_indices: &[SkillIndex]) -> RoutingStrategy {
        if let Some(skill) = skill_indices.iter().find(|s| s.matches_command(input)) {
            RoutingStrategy::Explicit {
                skill_name: skill.name.clone(),
            }
        } else {
            RoutingStrategy::NoSkill
        }
    }

    /// Route with index-based discovery
    fn route_index_based(&self, input: &str, skill_indices: &[SkillIndex]) -> RoutingStrategy {
        // 1. Check for explicit slash command
        if let Some(skill) = skill_indices.iter().find(|s| s.matches_command(input)) {
            return RoutingStrategy::Explicit {
                skill_name: skill.name.clone(),
            };
        }

        // 2. Check for trigger keyword matches
        let matches: Vec<_> = skill_indices
            .iter()
            .filter(|s| s.matches_triggers(input))
            .collect();

        if !matches.is_empty() {
            let matched_triggers: Vec<String> = matches
                .iter()
                .flat_map(|s| {
                    s.triggers
                        .iter()
                        .filter(|t| input.to_lowercase().contains(&t.to_lowercase()))
                        .cloned()
                })
                .collect();

            let skill_names: Vec<String> = matches.iter().map(|s| s.name.clone()).collect();

            return RoutingStrategy::KeywordMatch {
                matched_triggers,
                skill_names,
            };
        }

        // 3. Fall back to semantic matching (Claude will decide)
        if !skill_indices.is_empty() {
            RoutingStrategy::Semantic { confidence: 0.0 }
        } else {
            RoutingStrategy::NoSkill
        }
    }
}

impl Default for SkillRouter {
    fn default() -> Self {
        Self::new(SkillDiscoveryMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::skill_index::SkillScope;

    fn test_skills() -> Vec<SkillIndex> {
        vec![
            SkillIndex::new("commit", "Create a git commit")
                .with_triggers(vec!["git commit".into(), "commit changes".into()])
                .with_scope(SkillScope::User),
            SkillIndex::new("review", "Review code changes")
                .with_triggers(vec!["code review".into(), "review pr".into()])
                .with_scope(SkillScope::Project),
        ]
    }

    #[test]
    fn test_explicit_routing() {
        let router = SkillRouter::default();
        let skills = test_skills();

        let result = router.route("/commit", &skills);
        assert!(
            matches!(result, RoutingStrategy::Explicit { skill_name } if skill_name == "commit")
        );
    }

    #[test]
    fn test_keyword_routing() {
        let router = SkillRouter::default();
        let skills = test_skills();

        let result = router.route("I want to git commit these changes", &skills);
        assert!(matches!(result, RoutingStrategy::KeywordMatch { .. }));

        if let RoutingStrategy::KeywordMatch { skill_names, .. } = result {
            assert!(skill_names.contains(&"commit".to_string()));
        }
    }

    #[test]
    fn test_no_match_semantic() {
        let router = SkillRouter::default();
        let skills = test_skills();

        let result = router.route("help me with this bug", &skills);
        assert!(matches!(result, RoutingStrategy::Semantic { .. }));
    }

    #[test]
    fn test_no_skill_empty_index() {
        let router = SkillRouter::default();
        let skills: Vec<SkillIndex> = vec![];

        let result = router.route("anything", &skills);
        assert!(matches!(result, RoutingStrategy::NoSkill));
    }

    #[test]
    fn test_explicit_only_mode() {
        let router = SkillRouter::new(SkillDiscoveryMode::ExplicitOnly);
        let skills = test_skills();

        // Explicit command works
        let result = router.route("/commit", &skills);
        assert!(matches!(result, RoutingStrategy::Explicit { .. }));

        // Keywords don't match in explicit-only mode
        let result = router.route("git commit these changes", &skills);
        assert!(matches!(result, RoutingStrategy::NoSkill));
    }

    #[test]
    fn test_routing_strategy_methods() {
        let explicit = RoutingStrategy::Explicit {
            skill_name: "test".to_string(),
        };
        assert!(explicit.is_explicit());
        assert!(explicit.has_skill());
        assert_eq!(explicit.primary_skill(), Some("test"));

        let no_skill = RoutingStrategy::NoSkill;
        assert!(!no_skill.is_explicit());
        assert!(!no_skill.has_skill());
        assert_eq!(no_skill.primary_skill(), None);
    }
}
