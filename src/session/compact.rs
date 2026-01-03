//! Compact (Conversation Summarization)

use serde::{Deserialize, Serialize};

use super::state::{Session, SessionMessage};
use super::{SessionError, SessionResult};
use crate::client::DEFAULT_SMALL_MODEL;
use crate::types::{CompactResult, ContentBlock, DEFAULT_COMPACT_THRESHOLD, Role};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactStrategy {
    pub enabled: bool,
    pub threshold_percent: f32,
    pub summary_model: String,
    pub keep_recent_messages: usize,
    pub max_summary_tokens: u32,
}

impl Default for CompactStrategy {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_percent: DEFAULT_COMPACT_THRESHOLD,
            summary_model: DEFAULT_SMALL_MODEL.to_string(),
            keep_recent_messages: 4,
            max_summary_tokens: 2000,
        }
    }
}

impl CompactStrategy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    pub fn with_threshold(mut self, percent: f32) -> Self {
        self.threshold_percent = percent.clamp(0.1, 0.95);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.summary_model = model.into();
        self
    }

    pub fn with_keep_recent(mut self, count: usize) -> Self {
        self.keep_recent_messages = count.max(2);
        self
    }
}

pub struct CompactExecutor {
    strategy: CompactStrategy,
}

impl CompactExecutor {
    pub fn new(strategy: CompactStrategy) -> Self {
        Self { strategy }
    }

    pub fn needs_compact(&self, current_tokens: u64, max_tokens: u64) -> bool {
        if !self.strategy.enabled {
            return false;
        }

        let threshold = (max_tokens as f32 * self.strategy.threshold_percent) as u64;
        current_tokens >= threshold
    }

    pub fn prepare_compact(&self, session: &Session) -> SessionResult<PreparedCompact> {
        if !self.strategy.enabled {
            return Err(SessionError::Compact {
                message: "Compact is disabled".to_string(),
            });
        }

        let messages = session.get_current_branch();
        let total = messages.len();

        if total <= self.strategy.keep_recent_messages {
            return Ok(PreparedCompact::NotNeeded);
        }

        let split_point = total - self.strategy.keep_recent_messages;
        let to_summarize: Vec<_> = messages[..split_point].to_vec();
        let to_keep: Vec<_> = messages[split_point..].to_vec();

        // Format messages for summarization
        let summary_prompt = self.format_for_summary(&to_summarize);

        Ok(PreparedCompact::Ready {
            summary_prompt,
            messages_to_remove: split_point,
            messages_to_keep: to_keep.into_iter().cloned().collect(),
        })
    }

    /// Apply a compact with the generated summary
    pub fn apply_compact(
        &self,
        session: &mut Session,
        summary: String,
        messages_to_keep: Vec<SessionMessage>,
    ) -> CompactResult {
        let original_count = session.messages.len();

        // Estimate saved tokens (rough: 4 chars per token)
        let removed_chars: usize = session.messages[..(original_count - messages_to_keep.len())]
            .iter()
            .map(|m| {
                m.content
                    .iter()
                    .filter_map(|c| c.as_text())
                    .map(|t| t.len())
                    .sum::<usize>()
            })
            .sum();
        let saved_tokens = (removed_chars / 4) as u64;

        // Clear old messages
        session.messages.clear();
        session.summary = Some(summary.clone());

        // Add summary as first message
        let summary_msg = SessionMessage::user(vec![ContentBlock::text(format!(
            "[Previous conversation summary]\n{}",
            summary
        ))]);
        session.messages.push(summary_msg);

        // Add kept messages
        for msg in messages_to_keep {
            session.messages.push(msg);
        }

        // Update leaf pointer
        if let Some(last) = session.messages.last() {
            session.current_leaf_id = Some(last.id.clone());
        }

        CompactResult::Compacted {
            original_count,
            new_count: session.messages.len(),
            saved_tokens: saved_tokens as usize,
            summary,
        }
    }

    /// Format messages for the summarization prompt
    fn format_for_summary(&self, messages: &[&SessionMessage]) -> String {
        let mut formatted = String::new();
        formatted.push_str(COMPACTION_SUMMARY_PROMPT);
        formatted.push_str("\n\n---\n\n");

        for msg in messages {
            let role = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            };

            formatted.push_str(&format!("{}:\n", role));

            for block in &msg.content {
                if let Some(text) = block.as_text() {
                    let display_text = if text.len() > 2000 {
                        format!("{}... [truncated]", &text[..2000])
                    } else {
                        text.to_string()
                    };
                    formatted.push_str(&display_text);
                    formatted.push('\n');
                }
            }
            formatted.push('\n');
        }

        formatted
    }

    /// Get the strategy
    pub fn strategy(&self) -> &CompactStrategy {
        &self.strategy
    }
}

const COMPACTION_SUMMARY_PROMPT: &str = r#"Create a detailed summary of this conversation that will allow the conversation to continue seamlessly.

Focus on preserving:
1. **User's Original Request**: The core task or question the user asked
2. **Key Decisions Made**: Architectural choices, design decisions, approach selections
3. **Files Read/Modified**: List all files that were read or edited with brief context
4. **Code Changes**: Specific functions, classes, or modules that were modified
5. **Current Progress**: What has been completed and what remains
6. **Errors & Solutions**: Any errors encountered and how they were resolved
7. **Important Context**: Technical constraints, user preferences, project structure insights

Format the summary as structured sections. Be concise but preserve all information needed to continue the work without re-reading files or re-making decisions.

If there are active tasks (todos), plans, or background operations, note their current state."#;

/// Prepared compact operation
#[derive(Debug)]
pub enum PreparedCompact {
    /// Compact is not needed
    NotNeeded,
    /// Ready to execute
    Ready {
        /// Prompt for generating summary
        summary_prompt: String,
        /// Number of messages to remove
        messages_to_remove: usize,
        /// Messages to keep
        messages_to_keep: Vec<SessionMessage>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::state::SessionConfig;

    fn create_test_session(message_count: usize) -> Session {
        let mut session = Session::new(SessionConfig::default());

        for i in 0..message_count {
            let content = if i % 2 == 0 {
                format!("User message {}", i)
            } else {
                format!("Assistant response {}", i)
            };

            let msg = if i % 2 == 0 {
                SessionMessage::user(vec![ContentBlock::text(content)])
            } else {
                SessionMessage::assistant(vec![ContentBlock::text(content)])
            };

            session.add_message(msg);
        }

        session
    }

    #[test]
    fn test_compact_strategy_default() {
        let strategy = CompactStrategy::default();
        assert!(strategy.enabled);
        assert_eq!(strategy.threshold_percent, 0.8);
        assert_eq!(strategy.keep_recent_messages, 4);
    }

    #[test]
    fn test_compact_strategy_disabled() {
        let strategy = CompactStrategy::disabled();
        assert!(!strategy.enabled);
    }

    #[test]
    fn test_needs_compact() {
        let executor = CompactExecutor::new(CompactStrategy::default().with_threshold(0.8));

        // Below threshold
        assert!(!executor.needs_compact(70_000, 100_000));

        // At threshold
        assert!(executor.needs_compact(80_000, 100_000));

        // Above threshold
        assert!(executor.needs_compact(90_000, 100_000));
    }

    #[test]
    fn test_prepare_compact_not_needed() {
        let session = create_test_session(3);
        let executor = CompactExecutor::new(CompactStrategy::default().with_keep_recent(4));

        let result = executor.prepare_compact(&session).unwrap();
        assert!(matches!(result, PreparedCompact::NotNeeded));
    }

    #[test]
    fn test_prepare_compact_ready() {
        let session = create_test_session(10);
        let executor = CompactExecutor::new(CompactStrategy::default().with_keep_recent(4));

        let result = executor.prepare_compact(&session).unwrap();

        match result {
            PreparedCompact::Ready {
                summary_prompt,
                messages_to_remove,
                messages_to_keep,
            } => {
                assert!(summary_prompt.contains("summary"));
                assert!(summary_prompt.contains("User's Original Request"));
                assert_eq!(messages_to_remove, 6);
                assert_eq!(messages_to_keep.len(), 4);
            }
            _ => panic!("Expected Ready"),
        }
    }

    #[test]
    fn test_apply_compact() {
        let mut session = create_test_session(10);
        let executor = CompactExecutor::new(CompactStrategy::default().with_keep_recent(4));

        let prepared = executor.prepare_compact(&session).unwrap();

        if let PreparedCompact::Ready {
            messages_to_keep, ..
        } = prepared
        {
            let result = executor.apply_compact(
                &mut session,
                "This is a test summary".to_string(),
                messages_to_keep,
            );

            match result {
                CompactResult::Compacted {
                    original_count,
                    new_count,
                    summary,
                    ..
                } => {
                    assert_eq!(original_count, 10);
                    assert_eq!(new_count, 5); // 1 summary + 4 kept
                    assert!(summary.contains("test summary"));
                }
                _ => panic!("Expected Compacted"),
            }

            // Check session state
            assert!(session.summary.is_some());
            assert!(
                session.messages[0].content[0]
                    .as_text()
                    .unwrap()
                    .contains("summary")
            );
        }
    }
}
