//! Conversation context management.

use crate::types::{Message, Usage};

const CHARS_PER_TOKEN: usize = 4;
const NON_TEXT_BLOCK_TOKENS: usize = 25;

/// Manages conversation history and context window
#[derive(Debug, Default)]
pub struct ConversationContext {
    messages: Vec<Message>,
    total_usage: Usage,
    estimated_tokens: usize,
}

impl ConversationContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a message to the context
    pub fn push(&mut self, message: Message) {
        let text_len: usize = message
            .content
            .iter()
            .map(|block| match block {
                crate::types::ContentBlock::Text { text } => text.len(),
                _ => NON_TEXT_BLOCK_TOKENS * CHARS_PER_TOKEN,
            })
            .sum();
        self.estimated_tokens += text_len / CHARS_PER_TOKEN;
        self.messages.push(message);
    }

    /// Update usage statistics
    pub fn update_usage(&mut self, usage: Usage) {
        self.total_usage.input_tokens += usage.input_tokens;
        self.total_usage.output_tokens += usage.output_tokens;
        if let Some(cache_read) = usage.cache_read_input_tokens {
            self.total_usage.cache_read_input_tokens = Some(
                self.total_usage.cache_read_input_tokens.unwrap_or(0) + cache_read,
            );
        }
    }

    /// Get all messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get messages as mutable
    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Get total usage
    pub fn total_usage(&self) -> &Usage {
        &self.total_usage
    }

    /// Get estimated token count
    pub fn estimated_tokens(&self) -> usize {
        self.estimated_tokens
    }

    /// Check if context should be compacted
    pub fn should_compact(&self, max_tokens: usize, threshold: f32) -> bool {
        self.estimated_tokens as f32 > max_tokens as f32 * threshold
    }

    /// Compact the context (placeholder for summarization)
    pub async fn compact(&mut self, _client: &crate::Client) -> crate::Result<()> {
        Ok(())
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.estimated_tokens = 0;
    }

    /// Get message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_push() {
        let mut ctx = ConversationContext::new();
        ctx.push(Message::user("Hello"));
        assert_eq!(ctx.len(), 1);
        assert!(ctx.estimated_tokens() > 0);
    }

    #[test]
    fn test_should_compact() {
        let mut ctx = ConversationContext::new();
        ctx.estimated_tokens = 8000;
        assert!(ctx.should_compact(10000, 0.75));
        assert!(!ctx.should_compact(10000, 0.85));
    }
}
