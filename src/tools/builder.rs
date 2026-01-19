//! Tool registry builder.

use std::path::PathBuf;
use std::sync::Arc;

use super::ProcessManager;
use super::access::ToolAccess;
use super::context::ExecutionContext;
use super::env::ToolExecutionEnv;
use super::registry::ToolRegistry;
use super::traits::Tool;
use crate::agent::{TaskOutputTool, TaskRegistry, TaskTool};
use crate::common::IndexRegistry;
use crate::permissions::PermissionPolicy;
use crate::session::session_state::ToolState;
use crate::session::{MemoryPersistence, SessionId};
use crate::subagents::SubagentIndex;

pub struct ToolRegistryBuilder<'a> {
    access: Option<&'a ToolAccess>,
    working_dir: Option<PathBuf>,
    task_registry: Option<TaskRegistry>,
    skill_executor: Option<crate::skills::SkillExecutor>,
    subagent_registry: Option<IndexRegistry<SubagentIndex>>,
    policy: Option<PermissionPolicy>,
    sandbox_config: Option<crate::security::SandboxConfig>,
    tool_state: Option<ToolState>,
    session_id: Option<SessionId>,
}

impl<'a> ToolRegistryBuilder<'a> {
    pub fn new() -> Self {
        Self {
            access: None,
            working_dir: None,
            task_registry: None,
            skill_executor: None,
            subagent_registry: None,
            policy: None,
            sandbox_config: None,
            tool_state: None,
            session_id: None,
        }
    }

    pub fn access(mut self, access: &'a ToolAccess) -> Self {
        self.access = Some(access);
        self
    }

    pub fn working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn task_registry(mut self, registry: TaskRegistry) -> Self {
        self.task_registry = Some(registry);
        self
    }

    pub fn skill_executor(mut self, executor: crate::skills::SkillExecutor) -> Self {
        self.skill_executor = Some(executor);
        self
    }

    pub fn subagent_registry(mut self, registry: IndexRegistry<SubagentIndex>) -> Self {
        self.subagent_registry = Some(registry);
        self
    }

    pub fn policy(mut self, policy: PermissionPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn sandbox_config(mut self, config: crate::security::SandboxConfig) -> Self {
        self.sandbox_config = Some(config);
        self
    }

    pub fn tool_state(mut self, state: ToolState) -> Self {
        self.tool_state = Some(state);
        self
    }

    pub fn session_id(mut self, id: SessionId) -> Self {
        self.session_id = Some(id);
        self
    }

    pub fn build(self) -> ToolRegistry {
        let access = self.access.unwrap_or(&ToolAccess::All);
        let wd = self
            .working_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let permission_policy = self.policy.unwrap_or_default();

        let sandbox_config = self.sandbox_config.unwrap_or_else(|| {
            crate::security::SandboxConfig::disabled().with_working_dir(wd.clone())
        });

        let security = crate::security::SecurityContext::builder()
            .root(&wd)
            .sandbox(sandbox_config)
            .build()
            .map(|mut security| {
                security.policy = crate::security::SecurityPolicy::new(permission_policy);
                security
            })
            .unwrap_or_else(|_| crate::security::SecurityContext::permissive());

        let context = ExecutionContext::new(security);
        let task_registry = self
            .task_registry
            .unwrap_or_else(|| TaskRegistry::new(Arc::new(MemoryPersistence::new())));
        let process_manager = Arc::new(ProcessManager::new());

        let session_id = self.session_id.unwrap_or_default();
        let tool_state = self
            .tool_state
            .unwrap_or_else(|| ToolState::new(session_id));

        let task_tool: Arc<dyn Tool> = match self.subagent_registry {
            Some(sr) => Arc::new(TaskTool::new(task_registry.clone()).with_subagent_registry(sr)),
            None => Arc::new(TaskTool::new(task_registry.clone())),
        };

        let skill_tool: Arc<dyn Tool> = match self.skill_executor {
            Some(executor) => Arc::new(crate::skills::SkillTool::new(executor)),
            None => Arc::new(crate::skills::SkillTool::with_defaults()),
        };

        let all_tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(super::ReadTool),
            Arc::new(super::WriteTool),
            Arc::new(super::EditTool),
            Arc::new(super::GlobTool),
            Arc::new(super::GrepTool),
            Arc::new(super::BashTool::with_process_manager(
                process_manager.clone(),
            )),
            Arc::new(super::KillShellTool::with_process_manager(
                process_manager.clone(),
            )),
            task_tool,
            Arc::new(TaskOutputTool::new(task_registry.clone())),
            Arc::new(super::TodoWriteTool::new(tool_state.clone(), session_id)),
            Arc::new(super::PlanTool::new(tool_state.clone())),
            skill_tool,
        ];

        let env = ToolExecutionEnv {
            context,
            tool_state: Some(tool_state),
            process_manager: Some(process_manager),
        };

        let mut registry = ToolRegistry::with_env(task_registry, env);

        for tool in all_tools {
            if access.is_allowed(tool.name()) {
                registry.register(tool);
            }
        }

        registry
    }
}

impl Default for ToolRegistryBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}
