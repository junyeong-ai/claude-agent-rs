//! AgentBuilder struct and configuration methods.

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

pub const DEFAULT_COMPACT_KEEP_MESSAGES: usize = 4;

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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

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

    pub fn oauth_config(mut self, config: OAuthConfig) -> Self {
        self.oauth_config = Some(config);
        self
    }

    pub fn supports_server_tools(&self) -> bool {
        self.auth_type
            .as_ref()
            .map(|a| a.supports_server_tools())
            .unwrap_or(true)
    }

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

    pub fn config(mut self, config: ProviderConfig) -> Self {
        self.provider_config = Some(config);
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.config.model.primary = model.into();
        self
    }

    pub fn small_model(mut self, model: impl Into<String>) -> Self {
        self.config.model.small = model.into();
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.config.model.max_tokens = tokens;
        self
    }

    pub fn tools(mut self, access: ToolAccess) -> Self {
        self.config.security.tool_access = access;
        self
    }

    pub fn tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.custom_tools.push(Arc::new(tool));
        self
    }

    pub fn working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.working_dir = Some(path.into());
        self
    }

    pub fn system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.prompt.system_prompt = Some(prompt.into());
        self
    }

    pub fn system_prompt_mode(mut self, mode: SystemPromptMode) -> Self {
        self.config.prompt.system_prompt_mode = mode;
        self
    }

    pub fn append_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.prompt.system_prompt_mode = SystemPromptMode::Append;
        self.config.prompt.system_prompt = Some(prompt.into());
        self
    }

    pub fn output_style(mut self, style: OutputStyle) -> Self {
        self.config.prompt.output_style = Some(style);
        self
    }

    pub fn output_schema(mut self, schema: serde_json::Value) -> Self {
        self.config.prompt.output_schema = Some(schema);
        self
    }

    pub fn structured_output<T: schemars::JsonSchema>(mut self) -> Self {
        let schema = schemars::schema_for!(T);
        self.config.prompt.output_schema = serde_json::to_value(schema).ok();
        self
    }

    pub fn max_iterations(mut self, max: usize) -> Self {
        self.config.execution.max_iterations = max;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.execution.timeout = Some(timeout);
        self
    }

    pub fn auto_compact(mut self, enabled: bool) -> Self {
        self.config.execution.auto_compact = enabled;
        self
    }

    pub fn compact_keep_messages(mut self, count: usize) -> Self {
        self.config.execution.compact_keep_messages = count;
        self
    }

    pub fn permission_policy(mut self, policy: PermissionPolicy) -> Self {
        self.config.security.permission_policy = policy;
        self
    }

    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.config.security.permission_policy.mode = mode;
        self
    }

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

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.security.env.insert(key.into(), value.into());
        self
    }

    pub fn envs(
        mut self,
        vars: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        for (k, v) in vars {
            self.config.security.env.insert(k.into(), v.into());
        }
        self
    }

    pub fn allow_domain(mut self, domain: impl Into<String>) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .network
            .allowed_domains
            .insert(domain.into());
        self
    }

    pub fn deny_domain(mut self, domain: impl Into<String>) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .network
            .blocked_domains
            .insert(domain.into());
        self
    }

    pub fn sandbox_enabled(mut self, enabled: bool) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .enabled = enabled;
        self
    }

    pub fn exclude_command(mut self, command: impl Into<String>) -> Self {
        self.sandbox_settings
            .get_or_insert_with(crate::config::SandboxSettings::default)
            .excluded_commands
            .push(command.into());
        self
    }

    pub fn max_budget_usd(mut self, amount: f64) -> Self {
        self.config.budget.max_cost_usd = Some(amount);
        self
    }

    pub fn tenant_id(mut self, id: impl Into<String>) -> Self {
        self.config.budget.tenant_id = Some(id.into());
        self
    }

    pub fn tenant_budget_manager(mut self, manager: TenantBudgetManager) -> Self {
        self.tenant_budget_manager = Some(manager);
        self
    }

    pub fn fallback_model(mut self, model: impl Into<String>) -> Self {
        self.config.budget.fallback_model = Some(model.into());
        self
    }

    pub fn fallback(mut self, config: FallbackConfig) -> Self {
        self.fallback_config = Some(config);
        self
    }

    pub fn session_manager(mut self, manager: crate::session::SessionManager) -> Self {
        self.session_manager = Some(manager);
        self
    }

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

    pub fn output_style_name(mut self, name: impl Into<String>) -> Self {
        self.output_style_name = Some(name.into());
        self
    }

    pub fn mcp_server(
        mut self,
        name: impl Into<String>,
        config: crate::mcp::McpServerConfig,
    ) -> Self {
        self.mcp_configs.insert(name.into(), config);
        self
    }

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

    pub fn mcp(mut self, manager: crate::mcp::McpManager) -> Self {
        self.mcp_manager = Some(std::sync::Arc::new(manager));
        self
    }

    pub fn shared_mcp(mut self, manager: std::sync::Arc<crate::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub fn skill_registry(mut self, registry: SkillRegistry) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    pub fn skill(mut self, skill: SkillDefinition) -> Self {
        self.skill_registry
            .get_or_insert_with(SkillRegistry::new)
            .register(skill);
        self
    }

    pub fn skill_index(mut self, index: SkillIndex) -> Self {
        self.skill_indices.push(index);
        self
    }

    pub fn rule_index(mut self, index: RuleIndex) -> Self {
        self.rule_indices.push(index);
        self
    }

    pub fn memory_content(mut self, level: MemoryLevel, content: impl Into<String>) -> Self {
        let provider = self
            .memory_provider
            .get_or_insert_with(LeveledMemoryProvider::new);
        provider.add_content(level, content);
        self
    }

    pub fn subagent_registry(mut self, registry: SubagentRegistry) -> Self {
        self.subagent_registry = Some(registry);
        self
    }

    pub fn subagent(mut self, subagent: SubagentDefinition) -> Self {
        self.subagent_registry
            .get_or_insert_with(SubagentRegistry::with_builtins)
            .register(subagent);
        self
    }

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

    pub fn messages(mut self, messages: Vec<crate::types::Message>) -> Self {
        self.initial_messages = Some(messages);
        self
    }

    pub fn hook<H: Hook + 'static>(mut self, hook: H) -> Self {
        self.hooks.register(hook);
        self
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
