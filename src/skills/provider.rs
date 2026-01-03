//! Skill provider implementations using common infrastructure.

use super::{SkillDefinition, SkillLoader};
use crate::common::{
    FileProvider, InMemoryProvider as GenericInMemoryProvider, SkillLookupStrategy,
};

pub type InMemorySkillProvider = GenericInMemoryProvider<SkillDefinition>;

pub type FileSkillProvider = FileProvider<SkillDefinition, SkillLoader, SkillLookupStrategy>;

pub fn file_skill_provider() -> FileSkillProvider {
    FileSkillProvider::new(SkillLoader::new(), SkillLookupStrategy)
}

pub type ChainSkillProvider = crate::common::ChainProvider<SkillDefinition>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Provider;
    use crate::skills::SkillSourceType;

    #[tokio::test]
    async fn test_in_memory_provider() {
        let skill = SkillDefinition::new("test", "Test skill", "Do something");
        let provider = InMemorySkillProvider::new().with_item(skill);

        let names = provider.list().await.unwrap();
        assert_eq!(names, vec!["test"]);

        let loaded = provider.get("test").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_chain_provider() {
        let low = InMemorySkillProvider::new()
            .with_item(SkillDefinition::new("shared", "Low", "Low content"))
            .with_priority(0);

        let high = InMemorySkillProvider::new()
            .with_item(SkillDefinition::new("shared", "High", "High content"))
            .with_priority(10);

        let chain = ChainSkillProvider::new().with(low).with(high);

        let skill = chain.get("shared").await.unwrap().unwrap();
        assert_eq!(skill.description, "High");
    }

    #[tokio::test]
    async fn test_file_provider() {
        let temp = tempfile::tempdir().unwrap();
        let provider = file_skill_provider()
            .with_path(temp.path())
            .with_priority(5)
            .with_source_type(SkillSourceType::Project);

        assert_eq!(provider.priority(), 5);
        assert_eq!(provider.paths().len(), 1);
    }

    #[tokio::test]
    async fn test_file_provider_load_skill() {
        let temp = tempfile::tempdir().unwrap();
        let skill_file = temp.path().join("test.skill.md");
        tokio::fs::write(
            &skill_file,
            r#"---
name: test-skill
description: A test skill
---

Skill content here.
"#,
        )
        .await
        .unwrap();

        let provider = file_skill_provider().with_path(temp.path());

        let skill = provider.get("test").await.unwrap();
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().name, "test-skill");
    }
}
