//! Skill registry - manages available skills.
//!
//! The registry provides methods to register, get, and list skills.

use std::collections::HashMap;
use std::path::Path;

use super::{SkillDefinition, SkillLoader, SkillSourceType};

/// Registry for managing available skills
#[derive(Debug, Default)]
pub struct SkillRegistry {
    /// Registered skills by name
    skills: HashMap<String, SkillDefinition>,
    /// Loader for skill files
    loader: SkillLoader,
}

impl SkillRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a skill
    pub fn register(&mut self, skill: SkillDefinition) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Get a skill by name
    pub fn get(&self, name: &str) -> Option<&SkillDefinition> {
        self.skills.get(name)
    }

    /// Get a skill by trigger pattern
    pub fn get_by_trigger(&self, input: &str) -> Option<&SkillDefinition> {
        self.skills.values().find(|s| s.matches_trigger(input))
    }

    /// List all skill names
    pub fn list(&self) -> Vec<&str> {
        self.skills.keys().map(String::as_str).collect()
    }

    /// List all skills
    pub fn skills(&self) -> impl Iterator<Item = &SkillDefinition> {
        self.skills.values()
    }

    /// Get skill count
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Remove a skill by name
    pub fn remove(&mut self, name: &str) -> Option<SkillDefinition> {
        self.skills.remove(name)
    }

    /// Clear all skills
    pub fn clear(&mut self) {
        self.skills.clear();
    }

    /// Load a skill from a file and register it
    pub async fn load_file(&mut self, path: &Path) -> crate::Result<()> {
        let skill = self.loader.load_file(path).await?;
        self.register(skill);
        Ok(())
    }

    /// Load skills from a directory and register them
    pub async fn load_directory(&mut self, dir: &Path) -> crate::Result<usize> {
        let skills = self.loader.load_directory(dir).await?;
        let count = skills.len();
        for skill in skills {
            self.register(skill);
        }
        Ok(count)
    }

    /// Load a skill from inline content and register it
    pub fn load_inline(&mut self, content: &str) -> crate::Result<()> {
        let skill = self.loader.load_inline(content)?;
        self.register(skill);
        Ok(())
    }

    /// Get skills by source type
    pub fn get_by_source(&self, source_type: SkillSourceType) -> Vec<&SkillDefinition> {
        self.skills
            .values()
            .filter(|s| s.source_type == source_type)
            .collect()
    }

    /// Create a registry with default built-in skills
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // Register some basic built-in skills
        registry.register(
            SkillDefinition::new(
                "commit",
                "Create a git commit with a well-formatted message",
                include_str!("../prompts/skills/commit.txt"),
            )
            .with_source_type(SkillSourceType::Builtin)
            .with_trigger("/commit"),
        );

        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basic() {
        let mut registry = SkillRegistry::new();

        let skill = SkillDefinition::new("test", "Test skill", "Content");
        registry.register(skill);

        assert_eq!(registry.len(), 1);
        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_list() {
        let mut registry = SkillRegistry::new();

        registry.register(SkillDefinition::new("skill1", "Skill 1", "Content 1"));
        registry.register(SkillDefinition::new("skill2", "Skill 2", "Content 2"));

        let names = registry.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"skill1"));
        assert!(names.contains(&"skill2"));
    }

    #[test]
    fn test_registry_trigger_lookup() {
        let mut registry = SkillRegistry::new();

        registry
            .register(SkillDefinition::new("commit", "Commit", "Content").with_trigger("/commit"));

        assert!(registry.get_by_trigger("/commit please").is_some());
        assert!(registry.get_by_trigger("something else").is_none());
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = SkillRegistry::new();
        registry.register(SkillDefinition::new("test", "Test", "Content"));

        assert!(registry.remove("test").is_some());
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_inline() {
        let mut registry = SkillRegistry::new();

        let content = r#"---
name: inline-skill
description: An inline skill
---

Skill content here.
"#;

        registry.load_inline(content).unwrap();
        assert!(registry.get("inline-skill").is_some());
    }
}
