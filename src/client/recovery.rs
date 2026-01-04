//! Stream recovery for resumable streaming responses.

use std::time::Instant;

use crate::types::{ContentBlock, Message, Role, ThinkingBlock};

#[derive(Debug, Clone)]
struct ThinkingBuffer {
    thinking: String,
    signature: Option<String>,
}

#[derive(Debug, Clone)]
struct ToolUseBuffer {
    id: String,
    name: String,
    partial_json: String,
}

#[derive(Debug, Clone, Default)]
pub struct StreamRecoveryState {
    completed_blocks: Vec<ContentBlock>,
    pending_text: Option<String>,
    pending_thinking: Option<ThinkingBuffer>,
    pending_tool_use: Option<ToolUseBuffer>,
    started_at: Option<Instant>,
}

impl StreamRecoveryState {
    pub fn new() -> Self {
        Self {
            started_at: Some(Instant::now()),
            ..Default::default()
        }
    }

    pub fn append_text(&mut self, text: &str) {
        self.pending_text
            .get_or_insert_with(String::new)
            .push_str(text);
    }

    pub fn append_thinking(&mut self, thinking: &str) {
        match &mut self.pending_thinking {
            Some(buf) => buf.thinking.push_str(thinking),
            None => {
                self.pending_thinking = Some(ThinkingBuffer {
                    thinking: thinking.to_string(),
                    signature: None,
                });
            }
        }
    }

    pub fn append_signature(&mut self, signature: &str) {
        if let Some(buf) = &mut self.pending_thinking {
            buf.signature
                .get_or_insert_with(String::new)
                .push_str(signature);
        }
    }

    pub fn start_tool_use(&mut self, id: String, name: String) {
        self.pending_tool_use = Some(ToolUseBuffer {
            id,
            name,
            partial_json: String::new(),
        });
    }

    pub fn append_tool_json(&mut self, json: &str) {
        if let Some(buf) = &mut self.pending_tool_use {
            buf.partial_json.push_str(json);
        }
    }

    pub fn complete_text_block(&mut self) {
        if let Some(text) = self.pending_text.take()
            && !text.is_empty()
        {
            self.completed_blocks.push(ContentBlock::Text {
                text,
                citations: None,
                cache_control: None,
            });
        }
    }

    pub fn complete_thinking_block(&mut self) {
        if let Some(buf) = self.pending_thinking.take()
            && !buf.thinking.is_empty()
        {
            self.completed_blocks
                .push(ContentBlock::Thinking(ThinkingBlock {
                    thinking: buf.thinking,
                    signature: buf.signature.unwrap_or_default(),
                }));
        }
    }

    pub fn complete_tool_use_block(&mut self) {
        if let Some(buf) = self.pending_tool_use.take() {
            let input = match serde_json::from_str(&buf.partial_json) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        tool_name = %buf.name,
                        tool_id = %buf.id,
                        partial_json_len = buf.partial_json.len(),
                        error = %e,
                        "Stream recovery: failed to parse partial tool JSON, using empty object"
                    );
                    serde_json::Value::Object(serde_json::Map::new())
                }
            };
            self.completed_blocks
                .push(ContentBlock::ToolUse(crate::types::ToolUseBlock {
                    id: buf.id,
                    name: buf.name,
                    input,
                }));
        }
    }

    pub fn build_continuation_messages(&self, original: &[Message]) -> Vec<Message> {
        let mut messages = original.to_vec();
        let mut content = self.completed_blocks.clone();

        if let Some(text) = &self.pending_text
            && !text.is_empty()
        {
            content.push(ContentBlock::Text {
                text: text.clone(),
                citations: None,
                cache_control: None,
            });
        }

        if let Some(buf) = &self.pending_thinking
            && !buf.thinking.is_empty()
        {
            content.push(ContentBlock::Thinking(ThinkingBlock {
                thinking: buf.thinking.clone(),
                signature: buf.signature.clone().unwrap_or_default(),
            }));
        }

        if let Some(buf) = &self.pending_tool_use {
            let input = match serde_json::from_str(&buf.partial_json) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        tool_name = %buf.name,
                        tool_id = %buf.id,
                        partial_json_len = buf.partial_json.len(),
                        error = %e,
                        "Stream continuation: failed to parse partial tool JSON, using empty object"
                    );
                    serde_json::Value::Object(serde_json::Map::new())
                }
            };
            content.push(ContentBlock::ToolUse(crate::types::ToolUseBlock {
                id: buf.id.clone(),
                name: buf.name.clone(),
                input,
            }));
        }

        if !content.is_empty() {
            messages.push(Message {
                role: Role::Assistant,
                content,
            });
        }

        messages
    }

    pub fn is_recoverable(&self) -> bool {
        !self.completed_blocks.is_empty()
            || self.pending_text.is_some()
            || self.pending_thinking.is_some()
            || self.pending_tool_use.is_some()
    }

    pub fn elapsed(&self) -> Option<std::time::Duration> {
        self.started_at.map(|t| t.elapsed())
    }

    pub fn completed_blocks(&self) -> &[ContentBlock] {
        &self.completed_blocks
    }

    pub fn has_pending(&self) -> bool {
        self.pending_text.is_some()
            || self.pending_thinking.is_some()
            || self.pending_tool_use.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_state() {
        let state = StreamRecoveryState::new();
        assert!(!state.is_recoverable());
        assert!(state.completed_blocks().is_empty());
    }

    #[test]
    fn test_text_accumulation() {
        let mut state = StreamRecoveryState::new();
        state.append_text("Hello");
        state.append_text(" World");
        state.complete_text_block();

        assert!(state.is_recoverable());
        assert_eq!(state.completed_blocks().len(), 1);
    }

    #[test]
    fn test_thinking_accumulation() {
        let mut state = StreamRecoveryState::new();
        state.append_thinking("Let me think");
        state.append_signature("sig123");
        state.complete_thinking_block();

        assert!(state.is_recoverable());
        assert_eq!(state.completed_blocks().len(), 1);
    }

    #[test]
    fn test_continuation_messages() {
        let mut state = StreamRecoveryState::new();
        state.append_text("Partial response");

        let original = vec![Message::user("Hello")];
        let continued = state.build_continuation_messages(&original);

        assert_eq!(continued.len(), 2);
        assert_eq!(continued[1].role, Role::Assistant);
    }

    #[test]
    fn test_tool_use_accumulation() {
        let mut state = StreamRecoveryState::new();
        state.start_tool_use("tool_1".into(), "search".into());
        state.append_tool_json(r#"{"query":"#);
        state.append_tool_json(r#"test"}"#);
        state.complete_tool_use_block();

        assert!(state.is_recoverable());
        assert_eq!(state.completed_blocks().len(), 1);
    }
}
