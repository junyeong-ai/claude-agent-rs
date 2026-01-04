//! Agent core structure and construction.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::config::AgentConfig;
use crate::Client;
use crate::budget::{BudgetTracker, TenantBudget};
use crate::context::PromptOrchestrator;
use crate::hooks::HookManager;
use crate::session::ToolState;
use crate::tools::{ToolRegistry, ToolRegistryBuilder};
use crate::types::Message;

pub struct Agent {
    pub(crate) client: Arc<Client>,
    pub(crate) config: Arc<AgentConfig>,
    pub(crate) tools: Arc<ToolRegistry>,
    pub(crate) hooks: Arc<HookManager>,
    pub(crate) session_id: Arc<str>,
    pub(crate) state: ToolState,
    pub(crate) orchestrator: Option<Arc<RwLock<PromptOrchestrator>>>,
    pub(crate) initial_messages: Option<Vec<Message>>,
    pub(crate) budget_tracker: Arc<BudgetTracker>,
    pub(crate) tenant_budget: Option<Arc<TenantBudget>>,
    pub(crate) mcp_manager: Option<Arc<crate::mcp::McpManager>>,
}

impl Agent {
    pub fn new(client: Client, config: AgentConfig) -> Self {
        let tools = ToolRegistry::default_tools(
            &config.security.tool_access,
            config.working_dir.clone(),
            Some(config.security.permission_policy.clone()),
        );
        Self::from_parts(
            Arc::new(client),
            Arc::new(config),
            Arc::new(tools),
            Arc::new(HookManager::new()),
            None,
        )
    }

    pub fn with_skills(
        client: Client,
        config: AgentConfig,
        skill_executor: crate::skills::SkillExecutor,
    ) -> Self {
        let mut builder = ToolRegistryBuilder::new()
            .access(&config.security.tool_access)
            .skill_executor(skill_executor)
            .policy(config.security.permission_policy.clone());

        if let Some(dir) = config.working_dir.clone() {
            builder = builder.working_dir(dir);
        }

        let tools = builder.build();
        Self::from_parts(
            Arc::new(client),
            Arc::new(config),
            Arc::new(tools),
            Arc::new(HookManager::new()),
            None,
        )
    }

    pub fn with_components(
        client: Client,
        config: AgentConfig,
        tools: ToolRegistry,
        hooks: HookManager,
    ) -> Self {
        Self::from_parts(
            Arc::new(client),
            Arc::new(config),
            Arc::new(tools),
            Arc::new(hooks),
            None,
        )
    }

    pub fn with_orchestrator(
        client: Client,
        config: AgentConfig,
        tools: Arc<ToolRegistry>,
        hooks: HookManager,
        orchestrator: PromptOrchestrator,
    ) -> Self {
        Self::from_parts(
            Arc::new(client),
            Arc::new(config),
            tools,
            Arc::new(hooks),
            Some(Arc::new(RwLock::new(orchestrator))),
        )
    }

    pub fn with_shared_tools(
        client: Client,
        config: AgentConfig,
        tools: Arc<ToolRegistry>,
        hooks: HookManager,
    ) -> Self {
        Self::from_parts(
            Arc::new(client),
            Arc::new(config),
            tools,
            Arc::new(hooks),
            None,
        )
    }

    fn from_parts(
        client: Arc<Client>,
        config: Arc<AgentConfig>,
        tools: Arc<ToolRegistry>,
        hooks: Arc<HookManager>,
        orchestrator: Option<Arc<RwLock<PromptOrchestrator>>>,
    ) -> Self {
        let budget_tracker = match config.budget.max_cost_usd {
            Some(max) => BudgetTracker::new(max),
            None => BudgetTracker::unlimited(),
        };

        let config = Self::resolve_model_aliases(config, &client);

        let state = tools
            .tool_state()
            .cloned()
            .unwrap_or_else(|| ToolState::new(crate::session::SessionId::new()));
        let session_id: Arc<str> = state.session_id().to_string().into();

        Self {
            client,
            config,
            tools,
            hooks,
            session_id,
            state,
            orchestrator,
            initial_messages: None,
            budget_tracker: Arc::new(budget_tracker),
            tenant_budget: None,
            mcp_manager: None,
        }
    }

    fn resolve_model_aliases(config: Arc<AgentConfig>, client: &Client) -> Arc<AgentConfig> {
        let model_config = &client.config().models;

        let primary = &config.model.primary;
        let resolved_primary = model_config.resolve_alias(primary);

        let small = &config.model.small;
        let resolved_small = model_config.resolve_alias(small);

        // Only create new Arc if aliases changed
        if resolved_primary != primary || resolved_small != small {
            let mut new_config = (*config).clone();
            if resolved_primary != primary {
                tracing::debug!(
                    alias = %primary,
                    resolved = %resolved_primary,
                    "Resolved primary model alias"
                );
                new_config.model.primary = resolved_primary.to_string();
            }
            if resolved_small != small {
                tracing::debug!(
                    alias = %small,
                    resolved = %resolved_small,
                    "Resolved small model alias"
                );
                new_config.model.small = resolved_small.to_string();
            }
            Arc::new(new_config)
        } else {
            config
        }
    }

    pub fn with_tenant_budget(mut self, budget: Arc<TenantBudget>) -> Self {
        self.tenant_budget = Some(budget);
        self
    }

    pub fn with_mcp_manager(mut self, manager: Arc<crate::mcp::McpManager>) -> Self {
        self.mcp_manager = Some(manager);
        self
    }

    pub fn mcp_manager(&self) -> Option<&Arc<crate::mcp::McpManager>> {
        self.mcp_manager.as_ref()
    }

    pub fn with_initial_messages(mut self, messages: Vec<Message>) -> Self {
        self.initial_messages = Some(messages);
        self
    }

    pub fn with_session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = id.into().into();
        self
    }

    #[must_use]
    pub fn builder() -> super::AgentBuilder {
        super::AgentBuilder::new()
    }

    pub fn with_model(model: impl Into<String>) -> super::AgentBuilder {
        super::AgentBuilder::new().model(model)
    }

    pub async fn default_agent() -> crate::Result<Self> {
        Self::builder().build().await
    }

    #[must_use]
    pub fn hooks(&self) -> &Arc<HookManager> {
        &self.hooks
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    #[must_use]
    pub fn client(&self) -> &Arc<Client> {
        &self.client
    }

    pub fn orchestrator(&self) -> Option<&Arc<RwLock<PromptOrchestrator>>> {
        self.orchestrator.as_ref()
    }

    #[must_use]
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    #[must_use]
    pub fn tools(&self) -> &Arc<ToolRegistry> {
        &self.tools
    }

    #[must_use]
    pub fn state(&self) -> &ToolState {
        &self.state
    }
}
