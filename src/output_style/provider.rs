//! Output style provider implementations using common infrastructure.

use super::{OutputStyle, OutputStyleLoader};
use crate::common::{
    FileProvider, InMemoryProvider as GenericInMemoryProvider, OutputStyleLookupStrategy,
};

pub type InMemoryOutputStyleProvider = GenericInMemoryProvider<OutputStyle>;

pub type FileOutputStyleProvider =
    FileProvider<OutputStyle, OutputStyleLoader, OutputStyleLookupStrategy>;

pub type ChainOutputStyleProvider = crate::common::ChainProvider<OutputStyle>;

pub fn file_output_style_provider() -> FileOutputStyleProvider {
    FileOutputStyleProvider::new(OutputStyleLoader::new(), OutputStyleLookupStrategy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::Provider;

    #[tokio::test]
    async fn test_in_memory_provider() {
        let style = OutputStyle::new("test", "Test style", "Do something");
        let provider = InMemoryOutputStyleProvider::new().with_item(style);

        let names = provider.list().await.unwrap();
        assert_eq!(names, vec!["test"]);

        let loaded = provider.get("test").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_chain_provider() {
        let low = InMemoryOutputStyleProvider::new()
            .with_item(OutputStyle::new("shared", "Low", "Low content"))
            .with_priority(0);

        let high = InMemoryOutputStyleProvider::new()
            .with_item(OutputStyle::new("shared", "High", "High content"))
            .with_priority(10);

        let chain = ChainOutputStyleProvider::new().with(low).with(high);

        let style = chain.get("shared").await.unwrap().unwrap();
        assert_eq!(style.description, "High");
    }

    #[tokio::test]
    async fn test_file_provider() {
        use crate::output_style::OutputStyleSourceType;

        let temp = tempfile::tempdir().unwrap();
        let provider = file_output_style_provider()
            .with_path(temp.path())
            .with_priority(5)
            .with_source_type(OutputStyleSourceType::Project);

        assert_eq!(provider.priority(), 5);
        assert_eq!(provider.paths().len(), 1);
    }

    #[tokio::test]
    async fn test_file_provider_load_style() {
        let temp = tempfile::tempdir().unwrap();
        let style_file = temp.path().join("custom.md");
        tokio::fs::write(
            &style_file,
            r#"---
name: custom-style
description: A custom style
---

Custom content here.
"#,
        )
        .await
        .unwrap();

        let provider = file_output_style_provider().with_path(temp.path());

        let style = provider.get("custom").await.unwrap();
        assert!(style.is_some());
        assert_eq!(style.unwrap().name, "custom-style");
    }
}
