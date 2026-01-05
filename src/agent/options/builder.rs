//! Fluent builder for configuring and constructing [`crate::Agent`] instances.
//!
//! # Overview
//!
//! `AgentBuilder` provides a chainable API for configuring all aspects of an agent:
//! model selection, tool access, security policies, budget limits, and more.
//!
//! # Example
//!
//! ```rust,no_run
//! use claude_agent::{Agent, ToolAccess, Auth};
//!
//! # async fn example() -> claude_agent::Result<()> {
//! let agent = Agent::builder()
//!     .auth(Auth::from_env()).await?
//!     .model("claude-sonnet-4-5")
//!     .tools(ToolAccess::all())
//!     .working_dir("./project")
//!     .max_iterations(50)
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::auth::{Credential, OAuthConfig};
use crate::budget::TenantBudgetManager;
use crate::client::{CloudProvider, FallbackConfig, ModelConfig, ProviderConfig};
use crate::context::{LeveledMemoryProvider, MemoryLevel, RuleIndex, SkillIndex};
use crate::hooks::{Hook, HookManager};
use crate::output_style::OutputStyle;
use crate::permissions::{PermissionMode, PermissionPolicy, PermissionRule};
use crate::skills::{SkillDefinition, SkillRegistry};
use crate::subagents::{SubagentDefinition, SubagentRegistry};
use crate::tools::{Tool, ToolAccess};

use crate::agent::config::{AgentConfig, SystemPromptMode};

/// Default number of messages to preserve during context compaction.
pub const DEFAULT_COMPACT_KEEP_MESSAGES: usize = 4;

/// Fluent builder for constructing [`crate::Agent`] instances with custom configuration.
///
/// Use [`crate::Agent::builder()`] to create a new builder instance.
#[derive(Default)]
pub struct AgentBuilder {
    pub(super) config: AgentConfig,
    pub(super) credential: Option<Credential>,
    pub(super) auth_type: Option<crate::auth::Auth>,
    pub(super) oauth_config: Option<OAuthConfig>,
    pub(super) cloud_provider: Option<CloudProvider>,
    pub(super) model_config: Option<ModelConfig>,
    pub(super) provider_config: Option<ProviderConfig>,
    pub(super) skill_registry: Option<SkillRegistry>,
    pub(super) subagent_registry: Option<SubagentRegistry>,
    pub(super) skill_indices: Vec<SkillIndex>,
    pub(super) rule_indices: Vec<RuleIndex>,
    pub(super) hooks: HookManager,
    pub(super) custom_tools: Vec<Arc<dyn Tool>>,
    pub(super) memory_provider: Option<LeveledMemoryProvider>,
    pub(super) sandbox_settings: Option<crate::config::SandboxSettings>,
    pub(super) initial_messages: Option<Vec<crate::types::Message>>,
    pub(super) resume_session_id: Option<String>,
    pub(super) resumed_session: Option<crate::session::Session>,
    pub(super) tenant_budget_manager: Option<TenantBudgetManager>,
    pub(super) fallback_config: Option<FallbackConfig>,
    pub(super) output_style_name: Option<String>,
    pub(super) mcp_configs: std::collections::HashMap<String, crate::mcp::McpServerConfig>,
    pub(super) mcp_manager: Option<std::sync::Arc<crate::mcp::McpManager>>,
    pub(super) session_manager: Option<crate::session::SessionManager>,

    #[cfg(feature = "aws")]
    pub(super) aws_region: Option<String>,
    #[cfg(feature = "gcp")]
    pub(super) gcp_project: Option<String>,
    #[cfg(feature = "gcp")]
    pub(super) gcp_region: Option<String>,
    #[cfg(feature = "azure")]
    pub(super) azure_resource: Option<String>,
}

impl AgentBuilder {
    /// Creates a new builder with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    // =========================================================================
    // Configuration
    // =========================================================================

    /// Sets the complete agent configuration, replacing all defaults.
    pub fn agent_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the API provider configuration (timeouts, beta features, etc.).
    pub fn provider_config(mut self, config: ProviderConfig) -> Self {
        self.provider_config = Some(config);
        self
    }

    // =========================================================================
    // Authentication
    // =========================================================================

    /// Configures authentication for the API.
    ///
    /// # Supported Methods
    /// - `Auth::from_env()` - Uses `ANTHROPIC_API_KEY` environment variable
    /// - `Auth::api_key("sk-...")` - Explicit API key
    /// - `Auth::claude_cli()` - Uses Claude CLI OAuth (requires `cli-integration` feature)
    /// - `Auth::bedrock("region")` - AWS Bedrock (requires `aws` feature)
    /// - `Auth::vertex("project", "region")` - GCP Vertex AI (requires `gcp` feature)
    ///
    /// # Example
    /// ```rust,no_run
    /// # use claude_agent::{Agent, Auth};
    /// # async fn example() -> claude_agent::Result<()> {
    /// let agent = Agent::builder()
    ///     .auth(Auth::from_env()).await?
    ///     .build().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn auth(mut self, auth: impl Into<crate::auth::Auth>) -> crate::Result<Self> {
        let auth = auth.into();

        #[allow(unreachable_patterns)]
        match &auth {
            #[cfg(feature = "aws")]
            crate::auth::Auth::Bedrock { region } => {
                self.cloud_provider = Some(CloudProvider::Bedrock);
                self.aws_region = Some(region.clone());
                self.model_config = Some(ModelConfig::bedrock());
                self = self.apply_provider_models();
            }
            #[cfg(feature = "gcp")]
            crate::auth::Auth::Vertex { project, region } => {
                self.cloud_provider = Some(CloudProvider::Vertex);
                self.gcp_project = Some(project.clone());
                self.gcp_region = Some(region.clone());
                self.model_config = Some(ModelConfig::vertex());
                self = self.apply_provider_models();
            }
            #[cfg(feature = "azure")]
            crate::auth::Auth::Foundry { resource } => {
                self.cloud_provider = Some(CloudProvider::Foundry);
                self.azure_resource = Some(resource.clone());
                self.model_config = Some(ModelConfig::foundry());
                self = self.apply_provider_models();
            }
            _ => {}
        }

        let credential = auth.resolve().await?;
        if !credential.is_default() {
            self.credential = Some(credential);
        }

        self.auth_type = Some(auth);

        if self.supports_server_tools() {
            self.config.server_tools = crate::agent::config::ServerToolsConfig::all();
        }

        Ok(self)
    }

    /// Sets OAuth configuration for token refresh.
    pub fn oauth_config(mut self, config: OAuthConfig) -> Self {
        self.oauth_config = Some(config);
        self
    }

    /// Returns whether the current auth method supports server-side tools.
    pub fn supports_server_tools(&self) -> bool {
        self.auth_type
            .as_ref()
            .map(|a| a.supports_server_tools())
            .unwrap_or(true)
    }

    // =========================================================================
    // Model Configuration
    // =========================================================================

    /// Sets both primary and small model configurations.
    pub fn models(mut self, config: ModelConfig) -> Self {
        self.model_config = Some(config.clone());
        self.config.model.primary = config.primary;
        self.config.model.small = config.small;
        self
    }

    #[cfg(any(feature = "aws", feature = "gcp", feature = "azure"))]
    fn apply_provider_models(mut self) -> Self {
        if let Some(ref config) = self.model_config {
            if self.config.model.primary
                == crate::agent::config::AgentModelConfig::default().primary
            {
                self.config.model.primary = config.primary.clone();
            }
            if self.config.model.small == crate::agent::config::AgentModelConfig::default().small {
                self.config.model.small = config.small.clone();
            }
        }
        self
    }

    /// Sets the primary model for main operations.
    ///
    /// Default: `claude-sonnet-4-5-20250514`
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.model.primary = model.into();
        self
    }

    /// Sets the smaller model for quick operations (e.g., subagents).
    ///
    /// Default: `claude-haiku-4-5-20251001`
    pub fn small_model(mut self, model: impl Into<String>) -> Self {
        self.config.model.small = model.into();
        self
    }

    /// Sets the maximum tokens per response.
    ///
    /// Default: [`DEFAULT_MAX_TOKENS`] (8192)
    ///
    /// Note: Values exceeding 8192 require the 128k beta feature, which is
    /// automatically enabled when using `ProviderConfig::with_max_tokens`.
    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.config.model.max_tokens = tokens;
        self
    }

    // =========================================================================
    // Tools
    // =========================================================================

    /// Sets tool access policy.
    ///
    /// # Options
    /// - `ToolAccess::all()` - Enable all built-in tools
    /// - `ToolAccess::none()` - Disable all tools
    /// - `ToolAccess::only(["Read", "Write"])` - Enable specific tools
    /// - `ToolAccess::except(["Bash"])` - Enable all except specific tools
    pub fn tools(mut self, access: ToolAccess) -> Self {
        self.config.security.tool_access = access;
        self
    }

    /// Registers a custom tool implementation.
    pub fn tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.custom_tools.push(Arc::new(tool));
        self
    }

    // =========================================================================
    // Execution
    // =========================================================================

    /// Sets the working directory for file operations.
    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.working_dir = Some(path.into());
        self
    }

    /// Sets the maximum number of agentic loop iterations.
    ///
    /// Default: `100`
    pub fn max_iterations(mut self, max: usize) -> Self {
        self.config.execution.max_iterations = max;
        self
    }

    /// Sets the overall execution timeout.
    ///
    /// Default: `300 seconds`
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.execution.timeout = Some(timeout);
        self
    }

    /// Enables or disables automatic context compaction.
    ///
    /// Default: `true`
    pub fn auto_compact(mut self, enabled: bool) -> Self {
        self.config.execution.auto_compact = enabled;
        self
    }

    /// Sets the number of messages to preserve during compaction.
    ///
    /// Default: `4`
    pub fn compact_keep_messages(mut self, count: usize) -> Self {
        self.config.execution.compact_keep_messages = count;
        self
    }

    // =========================================================================
    // Prompts
    // =========================================================================

    /// Sets a custom system prompt, replacing the default.
    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.prompt.system_prompt = Some(prompt.into());
        self
    }

    /// Sets how the system prompt is applied.
    pub fn system_prompt_mode(mut self, mode: SystemPromptMode) -> Self {
        self.config.prompt.system_prompt_mode = mode;
        self
    }

    /// Appends to the default system prompt instead of replacing it.
    pub fn append_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.prompt.system_prompt_mode = SystemPromptMode::Append;
        self.config.prompt.system_prompt = Some(prompt.into());
        self
    }

    /// Sets the output style for response formatting.
    pub fn output_style(mut self, style: OutputStyle) -> Self {
        self.config.prompt.output_style = Some(style);
        self
    }

    /// Sets the output style by name (loaded from configuration).
    pub fn output_style_name(mut self, name: impl Into<String>) -> Self {
        self.output_style_name = Some(name.into());
        self
    }

    /// Sets a JSON schema for structured output.
    pub fn output_schema(mut self, schema: serde_json::Value) -> Self {
        self.config.prompt.output_schema = Some(schema);
        self
    }

    /// Enables structured output with automatic schema generation.
    pub fn structured_output<T: schemars::JsonSchema>(mut self) -> Self {
        let schema = schemars::schema_for!(T);
        self.config.prompt.output_schema = serde_json::to_value(schema).ok();
        self
    }

    // =========================================================================
    // Permissions
    // =========================================================================

    /// Sets the complete permission policy.
    pub fn permission_policy(mut self, policy: PermissionPolicy) -> Self {
        self.config.security.permission_policy = policy;
        self
    }

    /// Sets the permission mode (permissive, default, or strict).
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.config.security.permission_policy.mode = mode;
        self
    }

    /// Adds a rule to allow a tool or pattern.
    pub fn allow_tool(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        let rule = if pattern.contains('(') && pattern.contains(')') {
            PermissionRule::allow_scoped(&pattern)
        } else {
            PermissionRule::allow(&pattern)
        };
        self.config.security.permission_policy.rules.push(rule);
        self
    }

    /// Adds a rule to deny a tool or pattern.
    pub fn deny_tool(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        let rule = if pattern.contains('(') && pattern.contains(')') {
            PermissionRule::deny_scoped(&pattern)
        } else {
            PermissionRule::deny(&pattern)
        };
        self.config.security.permission_policy.rules.push(rule);
        self
    }

    // =========================================================================
    // Environment
    // =========================================================================

    /// Sets an environment variable for tool execution.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.security.env.insert(key.into(), value.into());
        self
    }

    /// Sets multiple environment variables for tool execution.
    pub fn envs(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in vars {
            self.config.security.env.insert(k.into(), v.into());
        }
        self
    }

    // =========================================================================
    // Sandbox & Network
    // =========================================================================

    /// Adds a domain to the network allowlist.
    pub fn allow_domain(mut self, domain: impl Into<String>) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .network
            .allowed_domains
            .insert(domain.into());
        self
    }

    /// Adds a domain to the network blocklist.
    pub fn deny_domain(mut self, domain: impl Into<String>) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .network
            .blocked_domains
            .insert(domain.into());
        self
    }

    /// Enables or disables sandbox isolation.
    pub fn sandbox_enabled(mut self, enabled: bool) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .enabled = enabled;
        self
    }

    /// Excludes a command from sandbox restrictions.
    pub fn exclude_command(mut self, command: impl Into<String>) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .excluded_commands
            .push(command.into());
        self
    }

    // =========================================================================
    // Budget
    // =========================================================================

    /// Sets the maximum budget in USD.
    pub fn max_budget_usd(mut self, amount: f64) -> Self {
        self.config.budget.max_cost_usd = Some(amount);
        self
    }

    /// Sets the tenant ID for multi-tenant budget tracking.
    pub fn tenant_id(mut self, id: impl Into<String>) -> Self {
        self.config.budget.tenant_id = Some(id.into());
        self
    }

    /// Sets a shared tenant budget manager.
    pub fn tenant_budget_manager(mut self, manager: TenantBudgetManager) -> Self {
        self.tenant_budget_manager = Some(manager);
        self
    }

    /// Sets the model to fall back to when budget is exceeded.
    pub fn fallback_model(mut self, model: impl Into<String>) -> Self {
        self.config.budget.fallback_model = Some(model.into());
        self
    }

    /// Sets the complete fallback configuration.
    pub fn fallback(mut self, config: FallbackConfig) -> Self {
        self.fallback_config = Some(config);
        self
    }

    // =========================================================================
    // Session
    // =========================================================================

    /// Sets a custom session manager for persistence.
    pub fn session_manager(mut self, manager: crate::session::SessionManager) -> Self {
        self.session_manager = Some(manager);
        self
    }

    /// Forks an existing session, creating a new branch.
    pub async fn fork_session(mut self, session_id: impl Into<String>) -> crate::Result<Self> {
        let manager = self.session_manager.take().unwrap_or_default();
        let session_id_str: String = session_id.into();
        let original_id = crate::session::SessionId::from(session_id_str);
        let forked = manager
            .fork(&original_id)
            .await
            .map_err(|e| crate::Error::Session(e.to_string()))?;

        self.initial_messages = Some(forked.to_api_messages());
        self.resume_session_id = Some(forked.id.to_string());
        self.session_manager = Some(manager);
        Ok(self)
    }

    /// Resumes an existing session by ID.
    pub async fn resume_session(mut self, session_id: impl Into<String>) -> crate::Result<Self> {
        let session_id_str: String = session_id.into();
        let id = crate::session::SessionId::from(session_id_str);
        let manager = self.session_manager.take().unwrap_or_default();
        let session = manager.get(&id).await?;

        let messages: Vec<crate::types::Message> = session
            .messages
            .iter()
            .map(|m| crate::types::Message {
                role: m.role,
                content: m.content.clone(),
            })
            .collect();

        self.initial_messages = Some(messages);
        self.resume_session_id = Some(id.to_string());
        self.resumed_session = Some(session);
        self.session_manager = Some(manager);
        Ok(self)
    }

    /// Sets initial messages for the conversation.
    pub fn messages(mut self, messages: Vec<crate::types::Message>) -> Self {
        self.initial_messages = Some(messages);
        self
    }

    // =========================================================================
    // MCP (Model Context Protocol)
    // =========================================================================

    /// Adds an MCP server configuration.
    pub fn mcp_server(
        mut self,
        name: impl Into<String>,
        config: crate::mcp::McpServerConfig,
    ) -> Self {
        self.mcp_configs.insert(name.into(), config);
        self
    }

    /// Adds an MCP server using stdio transport.
    pub fn mcp_stdio(
        mut self,
        name: impl Into<String>,
        command: impl Into<String>,
        args: Vec<String>,
    ) -> Self {
        self.mcp_configs.insert(
            name.into(),
            crate::mcp::McpServerConfig::Stdio {
                command: command.into(),
                args,
                env: std::collections::HashMap::new(),
            },
        );
        self
    }

    /// Sets an owned MCP manager.
    pub fn mcp_manager(mut self, manager: crate::mcp::McpManager) -> Self {
        self.mcp_manager = Some(std::sync::Arc::new(manager));
        self
    }

    /// Sets a shared MCP manager (for multi-agent scenarios).
    pub fn shared_mcp_manager(mut self, manager: std::sync::Arc<crate::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    // =========================================================================
    // Skills
    // =========================================================================

    /// Sets a complete skill registry.
    pub fn skill_registry(mut self, registry: SkillRegistry) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    /// Registers a single skill.
    pub fn skill(mut self, skill: SkillDefinition) -> Self {
        self.skill_registry
            .get_or_insert_with(SkillRegistry::new)
            .register(skill);
        self
    }

    /// Adds a skill index for skill discovery.
    pub fn skill_index(mut self, index: SkillIndex) -> Self {
        self.skill_indices.push(index);
        self
    }

    /// Adds a rule index for rule discovery.
    pub fn rule_index(mut self, index: RuleIndex) -> Self {
        self.rule_indices.push(index);
        self
    }

    /// Adds memory content at a specific level.
    pub fn memory_content(mut self, level: MemoryLevel, content: impl Into<String>) -> Self {
        let provider = self
            .memory_provider
            .get_or_insert_with(LeveledMemoryProvider::new);
        provider.add_content(level, content);
        self
    }

    // =========================================================================
    // Subagents
    // =========================================================================

    /// Sets a complete subagent registry.
    pub fn subagent_registry(mut self, registry: SubagentRegistry) -> Self {
        self.subagent_registry = Some(registry);
        self
    }

    /// Registers a single subagent.
    pub fn subagent(mut self, subagent: SubagentDefinition) -> Self {
        self.subagent_registry
            .get_or_insert_with(SubagentRegistry::with_builtins)
            .register(subagent);
        self
    }

    // =========================================================================
    // Hooks
    // =========================================================================

    /// Registers an event hook.
    pub fn hook<H: Hook + 'static>(mut self, hook: H) -> Self {
        self.hooks.register(hook);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::DEFAULT_MAX_TOKENS;

    #[test]
    fn test_tool_access() {
        assert!(ToolAccess::all().is_allowed("Read"));
        assert!(!ToolAccess::none().is_allowed("Read"));
        assert!(ToolAccess::only(["Read", "Write"]).is_allowed("Read"));
        assert!(!ToolAccess::only(["Read", "Write"]).is_allowed("Bash"));
        assert!(!ToolAccess::except(["Bash"]).is_allowed("Bash"));
        assert!(ToolAccess::except(["Bash"]).is_allowed("Read"));
    }

    #[test]
    fn test_max_tokens_default() {
        let builder = AgentBuilder::new();
        assert_eq!(builder.config.model.max_tokens, DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn test_max_tokens_custom() {
        let builder = AgentBuilder::new().max_tokens(16384);
        assert_eq!(builder.config.model.max_tokens, 16384);
    }
}
