//! Input queue for handling concurrent user inputs.

use std::collections::VecDeque;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::state::SessionId;
use super::types::EnvironmentContext;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedInput {
    pub id: Uuid,
    pub session_id: SessionId,
    pub content: String,
    pub environment: Option<EnvironmentContext>,
    pub created_at: DateTime<Utc>,
}

impl QueuedInput {
    pub fn new(session_id: SessionId, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            content: content.into(),
            environment: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_environment(mut self, env: EnvironmentContext) -> Self {
        self.environment = Some(env);
        self
    }
}

#[derive(Clone, Debug)]
pub struct MergedInput {
    pub ids: Vec<Uuid>,
    pub content: String,
    pub environment: Option<EnvironmentContext>,
}

#[derive(Debug, Default)]
pub struct InputQueue {
    items: VecDeque<QueuedInput>,
}

impl InputQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, input: QueuedInput) -> Uuid {
        let id = input.id;
        self.items.push_back(input);
        id
    }

    pub fn cancel(&mut self, id: Uuid) -> Option<QueuedInput> {
        if let Some(pos) = self.items.iter().position(|i| i.id == id) {
            self.items.remove(pos)
        } else {
            None
        }
    }

    pub fn cancel_all(&mut self) -> Vec<QueuedInput> {
        self.items.drain(..).collect()
    }

    pub fn pending(&self) -> Vec<&QueuedInput> {
        self.items.iter().collect()
    }

    pub fn pending_count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn merge_all(&mut self) -> Option<MergedInput> {
        if self.items.is_empty() {
            return None;
        }

        let items: Vec<QueuedInput> = self.items.drain(..).collect();
        let ids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
        let environment = items.last().and_then(|i| i.environment.clone());

        let content = items
            .into_iter()
            .map(|i| i.content)
            .collect::<Vec<_>>()
            .join("\n");

        Some(MergedInput {
            ids,
            content,
            environment,
        })
    }

    pub fn dequeue(&mut self) -> Option<QueuedInput> {
        self.items.pop_front()
    }
}

#[derive(Clone)]
pub struct SharedInputQueue {
    inner: Arc<RwLock<InputQueue>>,
}

impl SharedInputQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(InputQueue::new())),
        }
    }

    pub async fn enqueue(&self, input: QueuedInput) -> Uuid {
        self.inner.write().await.enqueue(input)
    }

    pub async fn cancel(&self, id: Uuid) -> Option<QueuedInput> {
        self.inner.write().await.cancel(id)
    }

    pub async fn cancel_all(&self) -> Vec<QueuedInput> {
        self.inner.write().await.cancel_all()
    }

    pub async fn pending_count(&self) -> usize {
        self.inner.read().await.pending_count()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }

    pub async fn merge_all(&self) -> Option<MergedInput> {
        self.inner.write().await.merge_all()
    }

    pub async fn dequeue(&self) -> Option<QueuedInput> {
        self.inner.write().await.dequeue()
    }

    pub async fn pending_ids(&self) -> Vec<Uuid> {
        self.inner
            .read()
            .await
            .pending()
            .iter()
            .map(|i| i.id)
            .collect()
    }
}

impl Default for SharedInputQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_enqueue_dequeue() {
        let mut queue = InputQueue::new();
        let session_id = SessionId::new();

        let input = QueuedInput::new(session_id, "Hello");
        let id = queue.enqueue(input);

        assert_eq!(queue.pending_count(), 1);

        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.id, id);
        assert_eq!(dequeued.content, "Hello");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_cancel() {
        let mut queue = InputQueue::new();
        let session_id = SessionId::new();

        let input1 = QueuedInput::new(session_id, "First");
        let id1 = queue.enqueue(input1);

        let input2 = QueuedInput::new(session_id, "Second");
        let _id2 = queue.enqueue(input2);

        assert_eq!(queue.pending_count(), 2);

        let cancelled = queue.cancel(id1);
        assert!(cancelled.is_some());
        assert_eq!(cancelled.unwrap().content, "First");
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_queue_merge_single() {
        let mut queue = InputQueue::new();
        let session_id = SessionId::new();

        let input = QueuedInput::new(session_id, "Only one");
        queue.enqueue(input);

        let merged = queue.merge_all().unwrap();
        assert_eq!(merged.ids.len(), 1);
        assert_eq!(merged.content, "Only one");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_merge_multiple() {
        let mut queue = InputQueue::new();
        let session_id = SessionId::new();

        queue.enqueue(QueuedInput::new(session_id, "First"));
        queue.enqueue(QueuedInput::new(session_id, "Second"));
        queue.enqueue(QueuedInput::new(session_id, "Third"));

        let merged = queue.merge_all().unwrap();
        assert_eq!(merged.ids.len(), 3);
        assert_eq!(merged.content, "First\nSecond\nThird");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_queue_merge_with_environment() {
        let mut queue = InputQueue::new();
        let session_id = SessionId::new();

        let env1 = EnvironmentContext {
            git_branch: Some("main".to_string()),
            ..Default::default()
        };
        let env2 = EnvironmentContext {
            git_branch: Some("feature".to_string()),
            ..Default::default()
        };

        queue.enqueue(QueuedInput::new(session_id, "First").with_environment(env1));
        queue.enqueue(QueuedInput::new(session_id, "Second").with_environment(env2));

        let merged = queue.merge_all().unwrap();
        assert_eq!(
            merged.environment.unwrap().git_branch,
            Some("feature".to_string())
        );
    }

    #[test]
    fn test_queue_merge_empty() {
        let mut queue = InputQueue::new();
        assert!(queue.merge_all().is_none());
    }

    #[test]
    fn test_queue_cancel_all() {
        let mut queue = InputQueue::new();
        let session_id = SessionId::new();

        queue.enqueue(QueuedInput::new(session_id, "First"));
        queue.enqueue(QueuedInput::new(session_id, "Second"));

        let cancelled = queue.cancel_all();
        assert_eq!(cancelled.len(), 2);
        assert!(queue.is_empty());
    }

    #[tokio::test]
    async fn test_shared_queue() {
        let queue = SharedInputQueue::new();
        let session_id = SessionId::new();

        let id = queue.enqueue(QueuedInput::new(session_id, "Test")).await;
        assert_eq!(queue.pending_count().await, 1);

        let cancelled = queue.cancel(id).await;
        assert!(cancelled.is_some());
        assert!(queue.is_empty().await);
    }
}
