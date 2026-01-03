//! Context Compaction (Claude Code CLI compatible)
//!
//! Summarizes the entire conversation when context exceeds threshold.

use serde::{Deserialize, Serialize};

use super::state::{Session, SessionMessage};
use super::types::CompactRecord;
use super::{SessionError, SessionResult};
use crate::client::DEFAULT_SMALL_MODEL;
use crate::types::{CompactResult, ContentBlock, DEFAULT_COMPACT_THRESHOLD, Role};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactStrategy {
    pub enabled: bool,
    pub threshold_percent: f32,
    pub summary_model: String,
    pub max_summary_tokens: u32,
}

impl Default for CompactStrategy {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_percent: DEFAULT_COMPACT_THRESHOLD,
            summary_model: DEFAULT_SMALL_MODEL.to_string(),
            max_summary_tokens: 4000,
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
        self.threshold_percent = percent.clamp(0.5, 0.95);
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.summary_model = model.into();
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
        if messages.is_empty() {
            return Ok(PreparedCompact::NotNeeded);
        }

        let summary_prompt = self.format_for_summary(&messages);

        Ok(PreparedCompact::Ready {
            summary_prompt,
            message_count: messages.len(),
        })
    }

    pub fn apply_compact(&self, session: &mut Session, summary: String) -> CompactResult {
        let original_count = session.messages.len();

        let removed_chars: usize = session
            .messages
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

        session.messages.clear();
        session.summary = Some(summary.clone());

        let summary_msg = SessionMessage::user(vec![ContentBlock::text(format!(
            "[Previous conversation summary]\n\n{}",
            summary
        ))])
        .as_compact_summary();

        session.add_message(summary_msg);

        CompactResult::Compacted {
            original_count,
            new_count: 1,
            saved_tokens: saved_tokens as usize,
            summary,
        }
    }

    pub fn record_compact(&self, session: &mut Session, result: &CompactResult) {
        if let CompactResult::Compacted {
            original_count,
            new_count,
            saved_tokens,
            summary,
        } = result
        {
            let record = CompactRecord::new(session.id)
                .with_counts(*original_count, *new_count)
                .with_summary(summary.clone())
                .with_saved_tokens(*saved_tokens);
            session.record_compact(record);
        }
    }

    fn format_for_summary(&self, messages: &[&SessionMessage]) -> String {
        let mut formatted = String::new();
        formatted.push_str(COMPACTION_PROMPT);
        formatted.push_str("\n\n---\n\n");

        for msg in messages {
            let role = match msg.role {
                Role::User => "Human",
                Role::Assistant => "Assistant",
            };

            formatted.push_str(&format!("**{}**:\n", role));

            for block in &msg.content {
                if let Some(text) = block.as_text() {
                    let display_text = if text.len() > 3000 {
                        format!("{}... [truncated]", &text[..3000])
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

    pub fn strategy(&self) -> &CompactStrategy {
        &self.strategy
    }
}

const COMPACTION_PROMPT: &str = r#"Summarize this conversation to continue seamlessly. Preserve:

1. **Original Request**: The core task or question
2. **Decisions Made**: Architecture, design, approach choices
3. **Files Modified**: List with brief context
4. **Code Changes**: Functions, modules modified
5. **Current Progress**: Completed work and remaining tasks
6. **Errors & Fixes**: Issues encountered and resolutions
7. **Key Context**: Constraints, preferences, project structure

Format as structured sections. Be concise but complete."#;

#[derive(Debug)]
pub enum PreparedCompact {
    NotNeeded,
    Ready {
        summary_prompt: String,
        message_count: usize,
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
    }

    #[test]
    fn test_compact_strategy_disabled() {
        let strategy = CompactStrategy::disabled();
        assert!(!strategy.enabled);
    }

    #[test]
    fn test_needs_compact() {
        let executor = CompactExecutor::new(CompactStrategy::default().with_threshold(0.8));

        assert!(!executor.needs_compact(70_000, 100_000));
        assert!(executor.needs_compact(80_000, 100_000));
        assert!(executor.needs_compact(90_000, 100_000));
    }

    #[test]
    fn test_prepare_compact_empty() {
        let session = Session::new(SessionConfig::default());
        let executor = CompactExecutor::new(CompactStrategy::default());

        let result = executor.prepare_compact(&session).unwrap();
        assert!(matches!(result, PreparedCompact::NotNeeded));
    }

    #[test]
    fn test_prepare_compact_ready() {
        let session = create_test_session(10);
        let executor = CompactExecutor::new(CompactStrategy::default());

        let result = executor.prepare_compact(&session).unwrap();

        match result {
            PreparedCompact::Ready {
                summary_prompt,
                message_count,
            } => {
                assert!(summary_prompt.contains("Original Request"));
                assert_eq!(message_count, 10);
            }
            _ => panic!("Expected Ready"),
        }
    }

    #[test]
    fn test_apply_compact() {
        let mut session = create_test_session(10);
        let executor = CompactExecutor::new(CompactStrategy::default());

        let result = executor.apply_compact(&mut session, "Test summary".to_string());

        match result {
            CompactResult::Compacted {
                original_count,
                new_count,
                ..
            } => {
                assert_eq!(original_count, 10);
                assert_eq!(new_count, 1);
            }
            _ => panic!("Expected Compacted"),
        }

        assert!(session.summary.is_some());
        assert_eq!(session.messages.len(), 1);
        assert!(session.messages[0].is_compact_summary);
    }
}
