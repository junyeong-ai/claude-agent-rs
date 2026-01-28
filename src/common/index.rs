//! Index trait for progressive disclosure pattern.
//!
//! An `Index` represents minimal metadata that is always loaded in context,
//! while the full content is loaded on-demand via `ContentSource`.

use async_trait::async_trait;

use super::{ContentSource, Named, SourceType};

/// Core trait for index entries in the progressive disclosure pattern.
///
/// Index entries contain minimal metadata (name, description) that is always
/// available in the system prompt, while full content is loaded on-demand.
///
/// # Token Efficiency
///
/// By keeping only metadata in context:
/// - 50 skills × ~20 tokens each = ~1,000 tokens (always loaded)
/// - vs 50 skills × ~500 tokens each = ~25,000 tokens (if fully loaded)
///
/// # Example Implementation
///
/// ```ignore
/// pub struct SkillIndex {
///     name: String,
///     description: String,
///     source: ContentSource,
///     source_type: SourceType,
/// }
///
/// impl Index for SkillIndex {
///     fn source(&self) -> &ContentSource { &self.source }
///     fn source_type(&self) -> SourceType { self.source_type }
///     fn to_summary_line(&self) -> String {
///         format!("- {}: {}", self.name, self.description)
///     }
/// }
/// ```
#[async_trait]
pub trait Index: Named + Clone + Send + Sync + 'static {
    /// Get the content source for lazy loading.
    fn source(&self) -> &ContentSource;

    /// Get the source type (builtin, user, project, managed).
    fn source_type(&self) -> SourceType;

    /// Get the priority for override ordering.
    ///
    /// Higher priority indices override lower priority ones with the same name.
    /// Default ordering: Project(20) > User(10) > Builtin(0)
    fn priority(&self) -> i32 {
        match self.source_type() {
            SourceType::Project => 20,
            SourceType::User => 10,
            SourceType::Managed => 5,
            SourceType::Builtin => 0,
            SourceType::Plugin => -5,
        }
    }

    /// Generate a summary line for context injection.
    ///
    /// This should be a compact representation suitable for system prompts.
    fn to_summary_line(&self) -> String;

    /// Load the full content from the source.
    ///
    /// This is the lazy-loading mechanism. Content is fetched only when needed.
    async fn load_content(&self) -> crate::Result<String> {
        self.source().load().await
    }

    /// Get a short description for this index entry.
    fn description(&self) -> &str {
        ""
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::common::Named;

    #[derive(Clone, Debug)]
    struct TestIndex {
        name: String,
        desc: String,
        source: ContentSource,
        source_type: SourceType,
    }

    impl Named for TestIndex {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[async_trait]
    impl Index for TestIndex {
        fn source(&self) -> &ContentSource {
            &self.source
        }

        fn source_type(&self) -> SourceType {
            self.source_type
        }

        fn to_summary_line(&self) -> String {
            format!("- {}: {}", self.name, self.desc)
        }

        fn description(&self) -> &str {
            &self.desc
        }
    }

    #[test]
    fn test_priority_ordering() {
        let builtin = TestIndex {
            name: "test".into(),
            desc: "desc".into(),
            source: ContentSource::in_memory(""),
            source_type: SourceType::Builtin,
        };

        let user = TestIndex {
            name: "test".into(),
            desc: "desc".into(),
            source: ContentSource::in_memory(""),
            source_type: SourceType::User,
        };

        let project = TestIndex {
            name: "test".into(),
            desc: "desc".into(),
            source: ContentSource::in_memory(""),
            source_type: SourceType::Project,
        };

        let plugin = TestIndex {
            name: "test".into(),
            desc: "desc".into(),
            source: ContentSource::in_memory(""),
            source_type: SourceType::Plugin,
        };

        assert!(project.priority() > user.priority());
        assert!(user.priority() > builtin.priority());
        assert!(builtin.priority() > plugin.priority());
        assert_eq!(plugin.priority(), -5);
    }

    #[tokio::test]
    async fn test_load_content() {
        let index = TestIndex {
            name: "test".into(),
            desc: "desc".into(),
            source: ContentSource::in_memory("full content here"),
            source_type: SourceType::User,
        };

        let content = index.load_content().await.unwrap();
        assert_eq!(content, "full content here");
    }

    #[test]
    fn test_summary_line() {
        let index = TestIndex {
            name: "commit".into(),
            desc: "Create git commits".into(),
            source: ContentSource::in_memory(""),
            source_type: SourceType::User,
        };

        assert_eq!(index.to_summary_line(), "- commit: Create git commits");
    }
}
