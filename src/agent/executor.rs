//! Agent core structure and construction.

use std::sync::Arc;

use tokio::sync::RwLock;

use super::config::AgentConfig;
use crate::Client;
use crate::budget::{BudgetTracker, TenantBudget};
use crate::context::PromptOrchestrator;
use crate::hooks::HookManager;
use crate::tools::{ToolRegistry, ToolRegistryBuilder};
use crate::types::Message;

pub struct Agent {
    pub(crate) client: Client,
    pub(crate) config: AgentConfig,
    pub(crate) tools: Arc<ToolRegistry>,
    pub(crate) hooks: HookManager,
    pub(crate) session_id: String,
    pub(crate) orchestrator: Option<Arc<RwLock<PromptOrchestrator>>>,
    pub(crate) initial_messages: Option<Vec<Message>>,
    pub(crate) budget_tracker: BudgetTracker,
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
        Self::from_parts(client, config, Arc::new(tools), HookManager::new(), None)
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
        Self::from_parts(client, config, Arc::new(tools), HookManager::new(), None)
    }

    pub fn with_components(
        client: Client,
        config: AgentConfig,
        tools: ToolRegistry,
        hooks: HookManager,
    ) -> Self {
        Self::from_parts(client, config, Arc::new(tools), hooks, None)
    }

    pub fn with_orchestrator(
        client: Client,
        config: AgentConfig,
        tools: Arc<ToolRegistry>,
        hooks: HookManager,
        orchestrator: PromptOrchestrator,
    ) -> Self {
        Self::from_parts(
            client,
            config,
            tools,
            hooks,
            Some(Arc::new(RwLock::new(orchestrator))),
        )
    }

    pub fn with_shared_tools(
        client: Client,
        config: AgentConfig,
        tools: Arc<ToolRegistry>,
        hooks: HookManager,
    ) -> Self {
        Self::from_parts(client, config, tools, hooks, None)
    }

    fn from_parts(
        client: Client,
        config: AgentConfig,
        tools: Arc<ToolRegistry>,
        hooks: HookManager,
        orchestrator: Option<Arc<RwLock<PromptOrchestrator>>>,
    ) -> Self {
        let budget_tracker = match config.budget.max_cost_usd {
            Some(max) => BudgetTracker::new(max),
            None => BudgetTracker::unlimited(),
        };

        Self {
            client,
            config,
            tools,
            hooks,
            session_id: uuid::Uuid::new_v4().to_string(),
            orchestrator,
            initial_messages: None,
            budget_tracker,
            tenant_budget: None,
            mcp_manager: None,
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
        self.session_id = id.into();
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
    pub fn hooks(&self) -> &HookManager {
        &self.hooks
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
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
}
