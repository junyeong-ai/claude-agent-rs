//! Tool state for thread-safe state access.

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use super::SessionResult;
use super::state::{Session, SessionConfig, SessionId};
use super::types::{CompactRecord, Plan, PlanStatus, TodoItem, ToolExecution};

const MAX_EXECUTION_LOG_SIZE: usize = 1000;

#[derive(Debug)]
struct ToolExecutionLog {
    entries: Mutex<Vec<ToolExecution>>,
}

impl Default for ToolExecutionLog {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolExecutionLog {
    fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::with_capacity(64)),
        }
    }

    async fn append(&self, exec: ToolExecution) {
        let mut entries = self.entries.lock().await;
        if entries.len() >= MAX_EXECUTION_LOG_SIZE {
            let drain_count = MAX_EXECUTION_LOG_SIZE / 4;
            entries.drain(..drain_count);
        }
        entries.push(exec);
    }

    async fn all(&self) -> Vec<ToolExecution> {
        self.entries.lock().await.clone()
    }

    async fn for_plan(&self, plan_id: Uuid) -> Vec<ToolExecution> {
        self.entries
            .lock()
            .await
            .iter()
            .filter(|e| e.plan_id == Some(plan_id))
            .cloned()
            .collect()
    }

    async fn len(&self) -> usize {
        self.entries.lock().await.len()
    }

    async fn clear(&self) {
        self.entries.lock().await.clear();
    }
}

#[derive(Debug)]
struct ToolStateInner {
    session: RwLock<Session>,
    executions: ToolExecutionLog,
}

impl ToolStateInner {
    fn new(session_id: SessionId) -> Self {
        Self {
            session: RwLock::new(Session::with_id(session_id, SessionConfig::default())),
            executions: ToolExecutionLog::new(),
        }
    }

    fn from_session(session: Session) -> Self {
        Self {
            session: RwLock::new(session),
            executions: ToolExecutionLog::new(),
        }
    }
}

/// Thread-safe tool state handle.
#[derive(Debug, Clone)]
pub struct ToolState(Arc<ToolStateInner>);

impl ToolState {
    pub fn new(session_id: SessionId) -> Self {
        Self(Arc::new(ToolStateInner::new(session_id)))
    }

    pub fn from_session(session: Session) -> Self {
        Self(Arc::new(ToolStateInner::from_session(session)))
    }

    #[inline]
    pub async fn session_id(&self) -> SessionId {
        self.0.session.read().await.id
    }

    pub async fn session(&self) -> Session {
        self.0.session.read().await.clone()
    }

    pub async fn update_session(&self, session: Session) {
        *self.0.session.write().await = session;
    }

    pub async fn enter_plan_mode(&self, name: Option<String>) -> SessionResult<Plan> {
        Ok(self.0.session.write().await.enter_plan_mode(name).clone())
    }

    pub async fn current_plan(&self) -> Option<Plan> {
        self.0.session.read().await.current_plan.clone()
    }

    pub async fn update_plan_content(&self, content: String) -> SessionResult<()> {
        self.0.session.write().await.update_plan_content(content);
        Ok(())
    }

    pub async fn exit_plan_mode(&self) -> SessionResult<Option<Plan>> {
        Ok(self.0.session.write().await.exit_plan_mode())
    }

    pub async fn cancel_plan(&self) -> SessionResult<Option<Plan>> {
        Ok(self.0.session.write().await.cancel_plan())
    }

    #[inline]
    pub async fn is_in_plan_mode(&self) -> bool {
        self.0.session.read().await.is_in_plan_mode()
    }

    pub async fn set_todos(&self, todos: Vec<TodoItem>) -> SessionResult<()> {
        self.0.session.write().await.set_todos(todos);
        Ok(())
    }

    pub async fn todos(&self) -> Vec<TodoItem> {
        self.0.session.read().await.todos.clone()
    }

    #[inline]
    pub async fn count_in_progress(&self) -> usize {
        self.0.session.read().await.todos_in_progress_count()
    }

    pub async fn record_tool_execution(&self, mut exec: ToolExecution) {
        let plan_id = {
            let session = self.0.session.read().await;
            if let Some(ref plan) = session.current_plan
                && plan.status == PlanStatus::Executing
            {
                Some(plan.id)
            } else {
                None
            }
        };
        exec.plan_id = plan_id;
        self.0.executions.append(exec).await;
    }

    pub async fn tool_executions(&self) -> Vec<ToolExecution> {
        self.0.executions.all().await
    }

    pub async fn tool_executions_for_plan(&self, plan_id: Uuid) -> Vec<ToolExecution> {
        self.0.executions.for_plan(plan_id).await
    }

    pub async fn execution_log_len(&self) -> usize {
        self.0.executions.len().await
    }

    pub async fn clear_execution_log(&self) {
        self.0.executions.clear().await;
    }

    pub async fn record_compact(&self, record: CompactRecord) {
        self.0.session.write().await.record_compact(record);
    }

    pub async fn compact_history(&self) -> Vec<CompactRecord> {
        self.0.session.read().await.compact_history.clone()
    }

    #[inline]
    pub async fn session_snapshot(&self) -> (SessionId, Vec<TodoItem>, Option<Plan>) {
        let session = self.0.session.read().await;
        (
            session.id,
            session.todos.clone(),
            session.current_plan.clone(),
        )
    }

    #[inline]
    pub async fn execution_state(&self) -> (SessionId, bool, usize) {
        let session = self.0.session.read().await;
        (
            session.id,
            session.is_in_plan_mode(),
            session.todos_in_progress_count(),
        )
    }

    pub async fn record_execution_with_todos(
        &self,
        mut exec: ToolExecution,
        todos: Option<Vec<TodoItem>>,
    ) {
        let plan_id = {
            let mut session = self.0.session.write().await;
            let plan_id = if let Some(ref plan) = session.current_plan
                && plan.status == PlanStatus::Executing
            {
                Some(plan.id)
            } else {
                None
            };
            if let Some(todos) = todos {
                session.set_todos(todos);
            }
            plan_id
        };
        exec.plan_id = plan_id;
        self.0.executions.append(exec).await;
    }
}

impl Default for ToolState {
    fn default() -> Self {
        Self::new(SessionId::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plan_lifecycle() {
        let state = ToolState::new(SessionId::new());

        let plan = state
            .enter_plan_mode(Some("Test Plan".to_string()))
            .await
            .unwrap();
        assert_eq!(plan.status, PlanStatus::Draft);
        assert!(state.is_in_plan_mode().await);

        state
            .update_plan_content("Step 1\nStep 2".to_string())
            .await
            .unwrap();

        let approved = state.exit_plan_mode().await.unwrap();
        assert!(approved.is_some());
        assert_eq!(approved.unwrap().status, PlanStatus::Approved);
    }

    #[tokio::test]
    async fn test_todos() {
        let session_id = SessionId::new();
        let state = ToolState::new(session_id);

        let todos = vec![
            TodoItem::new(session_id, "Task 1", "Doing task 1"),
            TodoItem::new(session_id, "Task 2", "Doing task 2"),
        ];

        state.set_todos(todos).await.unwrap();
        let loaded = state.todos().await;
        assert_eq!(loaded.len(), 2);
    }

    #[tokio::test]
    async fn test_tool_execution_recording() {
        let session_id = SessionId::new();
        let state = ToolState::new(session_id);

        let exec = ToolExecution::new(session_id, "Bash", serde_json::json!({"command": "ls"}))
            .with_output("file1\nfile2", false)
            .with_duration(100);

        state.record_tool_execution(exec).await;

        let executions = state.tool_executions().await;
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].tool_name, "Bash");
    }

    #[tokio::test]
    async fn test_session_persistence_ready() {
        let session_id = SessionId::new();
        let state = ToolState::new(session_id);

        let todos = vec![TodoItem::new(session_id, "Task 1", "Doing task 1")];
        state.set_todos(todos).await.unwrap();
        state
            .enter_plan_mode(Some("My Plan".to_string()))
            .await
            .unwrap();

        let session = state.session().await;
        assert_eq!(session.todos.len(), 1);
        assert!(session.current_plan.is_some());
        assert_eq!(
            session.current_plan.unwrap().name,
            Some("My Plan".to_string())
        );
    }

    #[tokio::test]
    async fn test_resume_from_session() {
        let session_id = SessionId::new();

        let mut session = Session::new(SessionConfig::default());
        session.id = session_id;
        session.set_todos(vec![TodoItem::new(
            session_id,
            "Resumed Task",
            "Working on it",
        )]);
        session.enter_plan_mode(Some("Resumed Plan".to_string()));

        let state = ToolState::from_session(session);

        let todos = state.todos().await;
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].content, "Resumed Task");

        let plan = state.current_plan().await;
        assert!(plan.is_some());
        assert_eq!(plan.unwrap().name, Some("Resumed Plan".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_execution_recording() {
        let session_id = SessionId::new();
        let state = ToolState::new(session_id);

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let state = state.clone();
                let sid = session_id;
                tokio::spawn(async move {
                    let exec =
                        ToolExecution::new(sid, format!("Tool{}", i), serde_json::json!({"id": i}));
                    state.record_tool_execution(exec).await;
                })
            })
            .collect();

        for h in handles {
            h.await.unwrap();
        }

        let executions = state.tool_executions().await;
        assert_eq!(executions.len(), 10);
    }
}
