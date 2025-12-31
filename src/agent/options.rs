//! Agent configuration options.

use std::path::PathBuf;
use std::time::Duration;

use crate::auth::{ChainProvider, ClaudeCliProvider, CredentialProvider, ExplicitProvider};
use crate::skills::{SkillDefinition, SkillExecutor, SkillRegistry};

/// Tool access configuration.
#[derive(Debug, Clone, Default)]
pub enum ToolAccess {
    /// No tools available.
    None,
    /// All built-in tools available.
    #[default]
    All,
    /// Only specified tools available.
    Only(Vec<String>),
    /// All except specified tools.
    Except(Vec<String>),
}

impl ToolAccess {
    /// Access for all tools.
    pub fn all() -> Self {
        Self::All
    }

    /// Access for no tools.
    pub fn none() -> Self {
        Self::None
    }

    /// Access for only specified tools.
    pub fn only(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Only(tools.into_iter().map(Into::into).collect())
    }

    /// Access for all except specified tools.
    pub fn except(tools: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Except(tools.into_iter().map(Into::into).collect())
    }

    /// Check if a tool is allowed.
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Only(allowed) => allowed.iter().any(|t| t == tool_name),
            Self::Except(denied) => !denied.iter().any(|t| t == tool_name),
        }
    }
}

/// Agent configuration options.
#[derive(Debug, Clone)]
pub struct AgentOptions {
    /// Model to use.
    pub model: String,
    /// Maximum tokens per response.
    pub max_tokens: u32,
    /// Tool access configuration.
    pub tool_access: ToolAccess,
    /// Working directory for file operations.
    pub working_dir: Option<PathBuf>,
    /// Custom system prompt.
    pub system_prompt: Option<String>,
    /// Maximum agent loop iterations.
    pub max_iterations: usize,
    /// Execution timeout.
    pub timeout: Option<Duration>,
    /// Enable automatic context compaction.
    pub auto_compact: bool,
    /// Token threshold for compaction.
    pub compact_threshold: f32,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-5-20250929".to_string(),
            max_tokens: 8192,
            tool_access: ToolAccess::All,
            working_dir: None,
            system_prompt: None,
            max_iterations: 100,
            timeout: Some(Duration::from_secs(600)),
            auto_compact: true,
            compact_threshold: 0.85,
        }
    }
}

/// Builder for agent configuration.
#[derive(Default)]
pub struct AgentBuilder {
    options: AgentOptions,
    credential_provider: Option<Box<dyn CredentialProvider>>,
    skill_registry: Option<SkillRegistry>,
    skills_dir: Option<PathBuf>,
}

impl AgentBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set API key for authentication.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.credential_provider = Some(Box::new(ExplicitProvider::api_key(key)));
        self
    }

    /// Set OAuth token for authentication.
    pub fn oauth_token(mut self, token: impl Into<String>) -> Self {
        self.credential_provider = Some(Box::new(ExplicitProvider::oauth(token)));
        self
    }

    /// Use Claude Code CLI credentials.
    pub fn from_claude_cli(mut self) -> Self {
        self.credential_provider = Some(Box::new(ClaudeCliProvider::new()));
        self
    }

    /// Auto-resolve credentials (environment → CLI).
    pub fn auto_resolve(mut self) -> Self {
        self.credential_provider = Some(Box::new(ChainProvider::default()));
        self
    }

    /// Use custom credential provider.
    pub fn credential_provider<P: CredentialProvider + 'static>(mut self, provider: P) -> Self {
        self.credential_provider = Some(Box::new(provider));
        self
    }

    /// Set the model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.options.model = model.into();
        self
    }

    /// Set max tokens.
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.options.max_tokens = tokens;
        self
    }

    /// Set tool access.
    pub fn tools(mut self, access: ToolAccess) -> Self {
        self.options.tool_access = access;
        self
    }

    /// Set working directory.
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.options.working_dir = Some(path.into());
        self
    }

    /// Set system prompt.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.options.system_prompt = Some(prompt.into());
        self
    }

    /// Set max iterations.
    pub fn max_iterations(mut self, max: usize) -> Self {
        self.options.max_iterations = max;
        self
    }

    /// Set timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = Some(timeout);
        self
    }

    /// Enable/disable auto compaction.
    pub fn auto_compact(mut self, enabled: bool) -> Self {
        self.options.auto_compact = enabled;
        self
    }

    /// Set a custom skill registry.
    pub fn skill_registry(mut self, registry: SkillRegistry) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    /// Register a skill.
    pub fn skill(mut self, skill: SkillDefinition) -> Self {
        if self.skill_registry.is_none() {
            self.skill_registry = Some(SkillRegistry::new());
        }
        if let Some(ref mut registry) = self.skill_registry {
            registry.register(skill);
        }
        self
    }

    /// Load skills from a directory (.claude/skills/).
    pub fn skills_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.skills_dir = Some(path.into());
        self
    }

    /// Get the configured skill executor.
    fn build_skill_executor(&mut self) -> SkillExecutor {
        let registry = self.skill_registry.take().unwrap_or_else(SkillRegistry::with_defaults);
        SkillExecutor::new(registry)
    }

    /// Build the agent.
    pub fn build(mut self) -> crate::Result<super::Agent> {
        // Build skill executor first (before moving self)
        let skill_executor = self.build_skill_executor();

        let mut client_builder = crate::Client::builder();

        if let Some(provider) = self.credential_provider {
            let credential = futures::executor::block_on(provider.resolve())?;
            client_builder = crate::Client::builder();
            match credential {
                crate::auth::Credential::ApiKey(key) => {
                    client_builder = client_builder.api_key(key);
                }
                crate::auth::Credential::OAuth(oauth) => {
                    client_builder = client_builder.oauth_token(oauth.access_token);
                }
            }
        }

        let client = client_builder.build()?;
        Ok(super::Agent::with_skills(client, self.options, skill_executor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_access() {
        assert!(ToolAccess::all().is_allowed("Read"));
        assert!(!ToolAccess::none().is_allowed("Read"));
        assert!(ToolAccess::only(["Read", "Write"]).is_allowed("Read"));
        assert!(!ToolAccess::only(["Read", "Write"]).is_allowed("Bash"));
        assert!(!ToolAccess::except(["Bash"]).is_allowed("Bash"));
        assert!(ToolAccess::except(["Bash"]).is_allowed("Read"));
    }
}
