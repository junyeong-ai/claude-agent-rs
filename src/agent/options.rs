//! Agent configuration options.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::auth::{ChainProvider, ClaudeCliProvider, CredentialProvider, ExplicitProvider};
use crate::context::{ChainMemoryProvider, ContextBuilder, MemoryProvider};
use crate::extension::{Extension, ExtensionContext, ExtensionRegistry};
use crate::hooks::{Hook, HookManager};
use crate::skills::{SkillDefinition, SkillExecutor, SkillRegistry};
use crate::tools::{Tool, ToolRegistry};

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
            compact_threshold: crate::types::DEFAULT_COMPACT_THRESHOLD,
        }
    }
}

/// Builder for agent configuration.
pub struct AgentBuilder {
    options: AgentOptions,
    credential_provider: Option<Box<dyn CredentialProvider>>,
    skill_registry: Option<SkillRegistry>,
    skills_dir: Option<PathBuf>,
    extensions: ExtensionRegistry,
    hooks: HookManager,
    custom_tools: Vec<Arc<dyn Tool>>,
    memory_providers: Vec<Box<dyn MemoryProvider>>,
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self {
            options: AgentOptions::default(),
            credential_provider: None,
            skill_registry: None,
            skills_dir: None,
            extensions: ExtensionRegistry::new(),
            hooks: HookManager::new(),
            custom_tools: Vec::new(),
            memory_providers: Vec::new(),
        }
    }
}

impl AgentBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets API key for authentication.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.credential_provider = Some(Box::new(ExplicitProvider::api_key(key)));
        self
    }

    /// Sets OAuth token for authentication.
    pub fn oauth_token(mut self, token: impl Into<String>) -> Self {
        self.credential_provider = Some(Box::new(ExplicitProvider::oauth(token)));
        self
    }

    /// Uses Claude Code CLI credentials.
    pub fn from_claude_cli(mut self) -> Self {
        self.credential_provider = Some(Box::new(ClaudeCliProvider::new()));
        self
    }

    /// Auto-resolves credentials (environment → CLI).
    pub fn auto_resolve(mut self) -> Self {
        self.credential_provider = Some(Box::new(ChainProvider::default()));
        self
    }

    /// Uses custom credential provider.
    pub fn credential_provider<P: CredentialProvider + 'static>(mut self, provider: P) -> Self {
        self.credential_provider = Some(Box::new(provider));
        self
    }

    /// Sets the model.
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.options.model = model.into();
        self
    }

    /// Sets max tokens.
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.options.max_tokens = tokens;
        self
    }

    /// Sets tool access configuration.
    pub fn tools(mut self, access: ToolAccess) -> Self {
        self.options.tool_access = access;
        self
    }

    /// Registers a custom tool.
    pub fn tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.custom_tools.push(Arc::new(tool));
        self
    }

    /// Sets working directory.
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.options.working_dir = Some(path.into());
        self
    }

    /// Sets system prompt.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.options.system_prompt = Some(prompt.into());
        self
    }

    /// Sets max iterations.
    pub fn max_iterations(mut self, max: usize) -> Self {
        self.options.max_iterations = max;
        self
    }

    /// Sets timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = Some(timeout);
        self
    }

    /// Enables/disables auto compaction.
    pub fn auto_compact(mut self, enabled: bool) -> Self {
        self.options.auto_compact = enabled;
        self
    }

    /// Sets a custom skill registry.
    pub fn skill_registry(mut self, registry: SkillRegistry) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    /// Registers a skill.
    pub fn skill(mut self, skill: SkillDefinition) -> Self {
        self.skill_registry
            .get_or_insert_with(SkillRegistry::new)
            .register(skill);
        self
    }

    /// Loads skills from a directory (.claude/skills/).
    pub fn skills_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.skills_dir = Some(path.into());
        self
    }

    /// Registers an extension.
    pub fn extension<E: Extension + 'static>(mut self, ext: E) -> Self {
        self.extensions.add(ext);
        self
    }

    /// Registers multiple extensions.
    pub fn extensions<I, E>(mut self, exts: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Extension + 'static,
    {
        for ext in exts {
            self.extensions.add(ext);
        }
        self
    }

    /// Registers a hook.
    pub fn hook<H: Hook + 'static>(mut self, hook: H) -> Self {
        self.hooks.register(hook);
        self
    }

    /// Adds a memory provider.
    pub fn memory_provider<M: MemoryProvider + 'static>(mut self, provider: M) -> Self {
        self.memory_providers.push(Box::new(provider));
        self
    }

    /// Builds the agent.
    pub fn build(mut self) -> crate::Result<super::Agent> {
        // Build client
        let client = self.build_client()?;

        // Initialize skill registry and executor
        let skill_registry = self
            .skill_registry
            .take()
            .unwrap_or_else(SkillRegistry::with_defaults);

        // Build tool registry with skills
        let skill_executor = SkillExecutor::new(skill_registry);
        let mut tools = ToolRegistry::with_skills(
            &self.options.tool_access,
            self.options.working_dir.clone(),
            skill_executor,
        );

        // Register custom tools first
        for tool in std::mem::take(&mut self.custom_tools) {
            tools.register(tool);
        }

        // Build extension context and execute extensions
        if !self.extensions.is_empty() {
            let mut memory = ChainMemoryProvider::new();
            let mut context_builder = ContextBuilder::new();
            let mut dummy_skills = SkillRegistry::new();

            let mut ext_ctx = ExtensionContext {
                tools: &mut tools,
                hooks: &mut self.hooks,
                skills: &mut dummy_skills,
                memory: &mut memory,
                context: &mut context_builder,
                options: &self.options,
            };

            self.extensions.build_all(&mut ext_ctx)?;
        }

        Ok(super::Agent::with_components(
            client,
            self.options,
            tools,
            self.hooks,
            self.extensions,
        ))
    }

    /// Builds the client from credentials.
    fn build_client(&self) -> crate::Result<crate::Client> {
        let mut builder = crate::Client::builder();

        if let Some(ref provider) = self.credential_provider {
            let credential = futures::executor::block_on(provider.resolve())?;
            match credential {
                crate::auth::Credential::ApiKey(key) => {
                    builder = builder.api_key(key);
                }
                crate::auth::Credential::OAuth(oauth) => {
                    builder = builder.oauth_token(oauth.access_token);
                }
            }
        }

        builder.build()
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
