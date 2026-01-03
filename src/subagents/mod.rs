mod builtin;
#[cfg(feature = "cli-integration")]
mod loader;
pub mod provider;

pub use crate::common::Provider as SubagentProviderTrait;
pub use builtin::{builtin_subagents, find_builtin};
#[cfg(feature = "cli-integration")]
pub use loader::SubagentLoader;
pub use provider::{ChainSubagentProvider, InMemorySubagentProvider};
#[cfg(feature = "cli-integration")]
pub use provider::{FileSubagentProvider, file_subagent_provider};

use serde::{Deserialize, Serialize};

use crate::client::{ModelConfig, ModelType};
use crate::common::{BaseRegistry, Named, RegistryItem, SourceType, ToolRestricted};

pub use crate::common::SourceType as SubagentSourceType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentDefinition {
    pub name: String,
    pub description: String,
    pub prompt: String,
    #[serde(default)]
    pub source_type: SubagentSourceType,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub model_type: Option<ModelType>,
    #[serde(default)]
    pub skills: Vec<String>,
}

impl SubagentDefinition {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            prompt: prompt.into(),
            source_type: SubagentSourceType::default(),
            tools: Vec::new(),
            model: None,
            model_type: None,
            skills: Vec::new(),
        }
    }

    pub fn with_source_type(mut self, source_type: SubagentSourceType) -> Self {
        self.source_type = source_type;
        self
    }

    pub fn with_tools(mut self, tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tools = tools.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_model_type(mut self, model_type: ModelType) -> Self {
        self.model_type = Some(model_type);
        self
    }

    pub fn with_skills(mut self, skills: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.skills = skills.into_iter().map(Into::into).collect();
        self
    }

    /// Resolve the model to use for this subagent.
    ///
    /// Supports both direct model IDs and aliases:
    /// - `"opus"` → resolves to reasoning model (e.g., claude-opus-4-5)
    /// - `"sonnet"` → resolves to primary model (e.g., claude-sonnet-4-5)
    /// - `"haiku"` → resolves to small model (e.g., claude-haiku-4-5)
    /// - Direct model ID → passed through unchanged
    ///
    /// Falls back to `model_type` if `model` is not set.
    pub fn resolve_model<'a>(&'a self, config: &'a ModelConfig) -> &'a str {
        if let Some(ref model) = self.model {
            return config.resolve_alias(model);
        }
        config.get(self.model_type.unwrap_or_default())
    }
}

impl Named for SubagentDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl ToolRestricted for SubagentDefinition {
    fn allowed_tools(&self) -> &[String] {
        &self.tools
    }
}

impl RegistryItem for SubagentDefinition {
    fn source_type(&self) -> SourceType {
        self.source_type
    }
}

#[cfg(feature = "cli-integration")]
pub type SubagentRegistry = BaseRegistry<SubagentDefinition, SubagentLoader>;

#[cfg(feature = "cli-integration")]
impl SubagentRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register_all(builtin_subagents());
        registry
    }

    pub async fn load_from_directories(
        &mut self,
        working_dir: Option<&std::path::Path>,
    ) -> crate::Result<()> {
        let builtins = InMemorySubagentProvider::new()
            .with_items(builtin_subagents())
            .with_priority(0)
            .with_source_type(SourceType::Builtin);

        let mut chain = ChainSubagentProvider::new().with(builtins);

        if let Some(dir) = working_dir {
            let project = file_subagent_provider()
                .with_project_path(dir)
                .with_priority(20)
                .with_source_type(SourceType::Project);
            chain = chain.with(project);
        }

        let user = file_subagent_provider()
            .with_user_path()
            .with_priority(10)
            .with_source_type(SourceType::User);
        let chain = chain.with(user);

        let loaded = chain.load_all().await?;
        self.register_all(loaded);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ToolRestricted;

    #[test]
    fn test_subagent_definition() {
        let subagent = SubagentDefinition::new("reviewer", "Code reviewer", "Review the code")
            .with_source_type(SubagentSourceType::Project)
            .with_tools(["Read", "Grep", "Glob"])
            .with_model("claude-haiku-4-5-20251001");

        assert_eq!(subagent.name, "reviewer");
        assert!(subagent.has_tool_restrictions());
        assert!(subagent.is_tool_allowed("Read"));
        assert!(!subagent.is_tool_allowed("Bash"));

        let config = ModelConfig::default();
        assert_eq!(subagent.resolve_model(&config), "claude-haiku-4-5-20251001");
    }

    #[test]
    fn test_tool_pattern_matching() {
        let subagent = SubagentDefinition::new("git-agent", "Git helper", "Help with git")
            .with_tools(["Bash(git:*)", "Read"]);

        assert!(subagent.is_tool_allowed("Bash"));
        assert!(subagent.is_tool_allowed("Read"));
        assert!(!subagent.is_tool_allowed("Write"));
    }

    #[test]
    fn test_no_tool_restrictions() {
        let subagent = SubagentDefinition::new("general", "General agent", "Do anything");

        assert!(!subagent.has_tool_restrictions());
        assert!(subagent.is_tool_allowed("Anything"));
    }

    #[test]
    fn test_resolve_model_with_alias() {
        let config = ModelConfig::default();

        // Test alias resolution
        let subagent =
            SubagentDefinition::new("fast", "Fast agent", "Be quick").with_model("haiku");
        assert!(subagent.resolve_model(&config).contains("haiku"));

        let subagent =
            SubagentDefinition::new("smart", "Smart agent", "Think deep").with_model("opus");
        assert!(subagent.resolve_model(&config).contains("opus"));

        let subagent = SubagentDefinition::new("balanced", "Balanced agent", "Be balanced")
            .with_model("sonnet");
        assert!(subagent.resolve_model(&config).contains("sonnet"));

        // Test direct model ID passthrough
        let subagent = SubagentDefinition::new("custom", "Custom agent", "Custom")
            .with_model("claude-custom-model-v1");
        assert_eq!(subagent.resolve_model(&config), "claude-custom-model-v1");

        // Test fallback to model_type
        let subagent = SubagentDefinition::new("typed", "Typed agent", "Use type")
            .with_model_type(ModelType::Small);
        assert!(subagent.resolve_model(&config).contains("haiku"));
    }
}
