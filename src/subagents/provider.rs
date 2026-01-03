//! Subagent provider implementations using common infrastructure.

use super::{SubagentDefinition, SubagentLoader};
use crate::common::{
    FileProvider, InMemoryProvider as GenericInMemoryProvider, SubagentLookupStrategy,
};

pub type InMemorySubagentProvider = GenericInMemoryProvider<SubagentDefinition>;

pub type FileSubagentProvider =
    FileProvider<SubagentDefinition, SubagentLoader, SubagentLookupStrategy>;

pub fn file_subagent_provider() -> FileSubagentProvider {
    FileSubagentProvider::new(SubagentLoader::new(), SubagentLookupStrategy)
}

pub type ChainSubagentProvider = crate::common::ChainProvider<SubagentDefinition>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Provider;
    use crate::subagents::builtin_subagents;

    #[tokio::test]
    async fn test_in_memory_provider() {
        let subagent = SubagentDefinition::new("test", "Test agent", "Do something");
        let provider = InMemorySubagentProvider::new().with_item(subagent);

        let names = provider.list().await.unwrap();
        assert_eq!(names, vec!["test"]);

        let loaded = provider.get("test").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_in_memory_with_builtins() {
        let provider = InMemorySubagentProvider::new().with_items(builtin_subagents());
        let names = provider.list().await.unwrap();

        assert!(names.contains(&"explore".to_string()));
        assert!(names.contains(&"plan".to_string()));
        assert!(names.contains(&"general".to_string()));
    }

    #[tokio::test]
    async fn test_chain_provider_priority() {
        let low = InMemorySubagentProvider::new()
            .with_item(SubagentDefinition::new("shared", "Low", "Low content"))
            .with_priority(0);

        let high = InMemorySubagentProvider::new()
            .with_item(SubagentDefinition::new("shared", "High", "High content"))
            .with_priority(10);

        let chain = ChainSubagentProvider::new().with(low).with(high);

        let subagent = chain.get("shared").await.unwrap().unwrap();
        assert_eq!(subagent.description, "High");
    }

    #[tokio::test]
    async fn test_file_provider() {
        use crate::subagents::SubagentSourceType;

        let temp = tempfile::tempdir().unwrap();
        let provider = file_subagent_provider()
            .with_path(temp.path())
            .with_priority(5)
            .with_source_type(SubagentSourceType::Project);

        assert_eq!(provider.priority(), 5);
        assert_eq!(provider.paths().len(), 1);
    }

    #[tokio::test]
    async fn test_file_provider_load_subagent() {
        let temp = tempfile::tempdir().unwrap();
        let subagent_file = temp.path().join("test.md");
        tokio::fs::write(
            &subagent_file,
            r#"---
name: test-agent
description: A test agent
---

Agent prompt here.
"#,
        )
        .await
        .unwrap();

        let provider = file_subagent_provider().with_path(temp.path());

        let subagent = provider.get("test").await.unwrap();
        assert!(subagent.is_some());
        assert_eq!(subagent.unwrap().name, "test-agent");
    }
}
