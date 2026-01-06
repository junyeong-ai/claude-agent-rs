mod builtin;
#[cfg(feature = "cli-integration")]
mod generator;
#[cfg(feature = "cli-integration")]
mod loader;
mod provider;

pub use builtin::{builtin_styles, default_style, explanatory_style, find_builtin, learning_style};
#[cfg(feature = "cli-integration")]
pub use generator::SystemPromptGenerator;
#[cfg(feature = "cli-integration")]
pub use loader::{OutputStyleFrontmatter, OutputStyleLoader};
pub use provider::InMemoryOutputStyleProvider;
#[cfg(feature = "cli-integration")]
pub use provider::{ChainOutputStyleProvider, FileOutputStyleProvider, file_output_style_provider};

use serde::{Deserialize, Serialize};

#[cfg(feature = "cli-integration")]
use crate::common::Provider;
use crate::common::{BaseRegistry, Named, RegistryItem, SourceType};

pub use crate::common::SourceType as OutputStyleSourceType;

/// Definition of an output style.
///
/// Output styles customize Claude's behavior by modifying the system prompt.
/// The `keep_coding_instructions` flag determines whether standard coding
/// instructions are retained (true) or replaced by the custom prompt (false).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputStyle {
    pub name: String,
    pub description: String,
    pub prompt: String,
    #[serde(default, alias = "source")]
    pub source_type: OutputStyleSourceType,
    #[serde(default, rename = "keep-coding-instructions")]
    pub keep_coding_instructions: bool,
}

impl OutputStyle {
    /// Create a new output style with the given name, description, and prompt.
    ///
    /// By default, `keep_coding_instructions` is `true` to match the behavior
    /// of the default style and `CompactStrategy::default()`.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            prompt: prompt.into(),
            source_type: OutputStyleSourceType::default(),
            keep_coding_instructions: true,
        }
    }

    pub fn with_source_type(mut self, source_type: OutputStyleSourceType) -> Self {
        self.source_type = source_type;
        self
    }

    pub fn with_keep_coding_instructions(mut self, keep: bool) -> Self {
        self.keep_coding_instructions = keep;
        self
    }

    pub fn is_default(&self) -> bool {
        self.name == "default" && self.prompt.is_empty()
    }
}

impl Named for OutputStyle {
    fn name(&self) -> &str {
        &self.name
    }
}

impl RegistryItem for OutputStyle {
    fn source_type(&self) -> SourceType {
        self.source_type
    }
}

#[cfg(feature = "cli-integration")]
pub type OutputStyleRegistry = BaseRegistry<OutputStyle, OutputStyleLoader>;

#[cfg(feature = "cli-integration")]
impl OutputStyleRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_all(builtin_styles());
        registry
    }

    pub async fn load_from_directories(
        &mut self,
        working_dir: Option<&std::path::Path>,
    ) -> crate::Result<()> {
        let builtins = InMemoryOutputStyleProvider::new()
            .with_items(builtin_styles())
            .with_priority(0)
            .with_source_type(SourceType::Builtin);

        let mut chain = ChainOutputStyleProvider::new().with(builtins);

        if let Some(dir) = working_dir {
            let project = file_output_style_provider()
                .with_project_path(dir)
                .with_priority(20)
                .with_source_type(SourceType::Project);
            chain = chain.with(project);
        }

        let user = file_output_style_provider()
            .with_user_path()
            .with_priority(10)
            .with_source_type(SourceType::User);
        let chain = chain.with(user);

        let loaded = chain.load_all().await?;
        self.register_all(loaded);
        Ok(())
    }
}

impl Default for OutputStyle {
    fn default() -> Self {
        default_style()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_style_new() {
        let style = OutputStyle::new("test", "A test style", "Test prompt");

        assert_eq!(style.name, "test");
        assert_eq!(style.description, "A test style");
        assert_eq!(style.prompt, "Test prompt");
        assert_eq!(style.source_type, OutputStyleSourceType::User);
        // Default is now true for consistency with CompactStrategy
        assert!(style.keep_coding_instructions);
    }

    #[test]
    fn test_output_style_builder() {
        let style = OutputStyle::new("custom", "Custom style", "Custom prompt")
            .with_source_type(OutputStyleSourceType::Project)
            .with_keep_coding_instructions(true);

        assert_eq!(style.source_type, OutputStyleSourceType::Project);
        assert!(style.keep_coding_instructions);
    }

    #[test]
    fn test_default_style() {
        let style = default_style();

        assert!(style.is_default());
        assert_eq!(style.name, "default");
        assert!(style.keep_coding_instructions);
    }

    #[test]
    fn test_source_type_display() {
        assert_eq!(OutputStyleSourceType::Builtin.to_string(), "builtin");
        assert_eq!(OutputStyleSourceType::User.to_string(), "user");
        assert_eq!(OutputStyleSourceType::Project.to_string(), "project");
        assert_eq!(OutputStyleSourceType::Managed.to_string(), "managed");
    }
}
