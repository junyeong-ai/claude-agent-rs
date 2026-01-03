mod commands;
mod executor;
#[cfg(feature = "cli-integration")]
mod loader;
pub mod provider;
mod skill_tool;

pub use crate::common::Provider as SkillProviderTrait;
pub use commands::{CommandLoader, SlashCommand};
pub use executor::{ExecutionMode, SkillExecutionCallback, SkillExecutor};
#[cfg(feature = "cli-integration")]
pub use loader::SkillLoader;
pub use provider::{ChainSkillProvider, InMemorySkillProvider};
#[cfg(feature = "cli-integration")]
pub use provider::{FileSkillProvider, file_skill_provider};
pub use skill_tool::SkillTool;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use crate::common::{BaseRegistry, Named, RegistryItem, SourceType, ToolRestricted};

fn markdown_link_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]*)\]\(([^)]+)\)").expect("valid markdown link regex"))
}

pub use crate::common::SourceType as SkillSourceType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub content: String,
    #[serde(default)]
    pub source_type: SkillSourceType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<PathBuf>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub arguments: Option<serde_json::Value>,
    #[serde(default, alias = "allowed-tools")]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
}

impl SkillDefinition {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            content: content.into(),
            source_type: SkillSourceType::User,
            base_dir: None,
            triggers: Vec::new(),
            arguments: None,
            allowed_tools: Vec::new(),
            model: None,
        }
    }

    pub fn with_source_type(mut self, source_type: SkillSourceType) -> Self {
        self.source_type = source_type;
        self
    }

    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    pub fn with_trigger(mut self, trigger: impl Into<String>) -> Self {
        self.triggers.push(trigger.into());
        self
    }

    pub fn with_arguments(mut self, schema: serde_json::Value) -> Self {
        self.arguments = Some(schema);
        self
    }

    pub fn with_allowed_tools(
        mut self,
        tools: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.allowed_tools = tools.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn matches_trigger(&self, input: &str) -> bool {
        self.triggers
            .iter()
            .any(|t| input.to_lowercase().contains(&t.to_lowercase()))
    }

    pub fn resolve_path(&self, relative: &str) -> Option<PathBuf> {
        self.base_dir.as_ref().map(|base| base.join(relative))
    }

    pub fn content_with_resolved_paths(&self) -> String {
        let Some(base) = &self.base_dir else {
            return self.content.clone();
        };
        resolve_markdown_paths(&self.content, base)
    }
}

impl Named for SkillDefinition {
    fn name(&self) -> &str {
        &self.name
    }
}

impl ToolRestricted for SkillDefinition {
    fn allowed_tools(&self) -> &[String] {
        &self.allowed_tools
    }
}

impl RegistryItem for SkillDefinition {
    fn source_type(&self) -> SourceType {
        self.source_type
    }
}

#[cfg(feature = "cli-integration")]
pub type SkillRegistry = BaseRegistry<SkillDefinition, SkillLoader>;

#[cfg(feature = "cli-integration")]
impl SkillRegistry {
    pub fn get_by_trigger(&self, input: &str) -> Option<&SkillDefinition> {
        self.items().find(|s| s.matches_trigger(input))
    }
}

fn resolve_markdown_paths(content: &str, base_dir: &Path) -> String {
    markdown_link_regex()
        .replace_all(content, |caps: &Captures| {
            let text = &caps[1];
            let path = &caps[2];

            if path.starts_with("http://") || path.starts_with("https://") || path.starts_with('/')
            {
                return caps[0].to_string();
            }

            let resolved = base_dir.join(path);
            format!("[{}]({})", text, resolved.display())
        })
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<PathBuf>,
}

impl SkillResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
            allowed_tools: Vec::new(),
            model: None,
            base_dir: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(message.into()),
            allowed_tools: Vec::new(),
            model: None,
            base_dir: None,
        }
    }

    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    pub fn with_base_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.base_dir = dir;
        self
    }

    pub fn has_tool_restrictions(&self) -> bool {
        !self.allowed_tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ToolRestricted;

    #[test]
    fn test_skill_definition() {
        let skill = SkillDefinition::new(
            "commit",
            "Create a git commit",
            "Analyze changes and create commit message",
        )
        .with_source_type(SkillSourceType::Builtin)
        .with_trigger("/commit");

        assert_eq!(skill.name, "commit");
        assert_eq!(skill.source_type, SkillSourceType::Builtin);
        assert!(skill.matches_trigger("/commit please"));
    }

    #[test]
    fn test_skill_result() {
        let success = SkillResult::success("Done!");
        assert!(success.success);
        assert!(success.error.is_none());

        let error = SkillResult::error("Failed");
        assert!(!error.success);
        assert!(error.error.is_some());
    }

    #[test]
    fn test_skill_allowed_tools() {
        let skill = SkillDefinition::new("reader", "Read files", "Read: $ARGUMENTS")
            .with_allowed_tools(["Read", "Grep", "Glob"]);

        assert!(skill.has_tool_restrictions());
        assert!(skill.is_tool_allowed("Read"));
        assert!(skill.is_tool_allowed("Grep"));
        assert!(!skill.is_tool_allowed("Bash"));
        assert!(!skill.is_tool_allowed("Write"));
    }

    #[test]
    fn test_skill_allowed_tools_pattern() {
        let skill = SkillDefinition::new("git-helper", "Git commands", "Git: $ARGUMENTS")
            .with_allowed_tools(["Bash(git:*)", "Read"]);

        assert!(skill.is_tool_allowed("Bash")); // Base tool name
        assert!(skill.is_tool_allowed("Read"));
        assert!(!skill.is_tool_allowed("Write"));
    }

    #[test]
    fn test_skill_no_restrictions() {
        let skill = SkillDefinition::new("any", "Any tools", "Do: $ARGUMENTS");

        assert!(!skill.has_tool_restrictions());
        assert!(skill.is_tool_allowed("Bash"));
        assert!(skill.is_tool_allowed("Read"));
        assert!(skill.is_tool_allowed("Anything"));
    }

    #[test]
    fn test_skill_model_override() {
        let skill = SkillDefinition::new("fast-task", "Quick task", "Do: $ARGUMENTS")
            .with_model("claude-haiku-4-5-20251001");

        assert_eq!(skill.model, Some("claude-haiku-4-5-20251001".to_string()));
    }

    #[test]
    fn test_skill_result_with_context() {
        let result = SkillResult::success("Output")
            .with_allowed_tools(vec!["Read".to_string(), "Grep".to_string()])
            .with_model(Some("claude-haiku-4-5-20251001".to_string()));

        assert!(result.has_tool_restrictions());
        assert_eq!(result.allowed_tools, vec!["Read", "Grep"]);
        assert_eq!(result.model, Some("claude-haiku-4-5-20251001".to_string()));
    }

    #[test]
    fn test_skill_base_dir() {
        let skill = SkillDefinition::new("reviewer", "Review code", "Review the code")
            .with_base_dir("/home/user/.claude/skills/reviewer");

        assert_eq!(
            skill.resolve_path("style-guide.md"),
            Some(PathBuf::from(
                "/home/user/.claude/skills/reviewer/style-guide.md"
            ))
        );
    }

    #[test]
    fn test_content_with_resolved_paths() {
        let content = r#"# Review Process
Check [style-guide.md](style-guide.md) for standards.
Also see [docs/api.md](docs/api.md).
External: [Rust Docs](https://doc.rust-lang.org)
Absolute: [config](/etc/config.md)"#;

        let skill = SkillDefinition::new("test", "Test", content).with_base_dir("/skills/test");

        let resolved = skill.content_with_resolved_paths();

        assert!(resolved.contains("[style-guide.md](/skills/test/style-guide.md)"));
        assert!(resolved.contains("[docs/api.md](/skills/test/docs/api.md)"));
        assert!(resolved.contains("[Rust Docs](https://doc.rust-lang.org)"));
        assert!(resolved.contains("[config](/etc/config.md)"));
    }

    #[test]
    fn test_content_without_base_dir() {
        let skill = SkillDefinition::new("test", "Test", "See [file.md](file.md)");
        let resolved = skill.content_with_resolved_paths();
        assert_eq!(resolved, "See [file.md](file.md)");
    }
}
