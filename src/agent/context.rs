//! Conversation context management.

use crate::types::{CompactResult, ContentBlock, Message, Role, Usage};

const CHARS_PER_TOKEN: usize = 4;
const NON_TEXT_BLOCK_TOKENS: usize = 25;
const KEEP_RECENT_MESSAGES: usize = 4;
const SUMMARY_MODEL: &str = "claude-haiku-4-5-20251001";

/// Manages conversation history and context window.
#[derive(Debug, Default)]
pub struct ConversationContext {
    messages: Vec<Message>,
    total_usage: Usage,
    estimated_tokens: usize,
    summary: Option<String>,
    compactions: usize,
}

impl ConversationContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a message to the context.
    pub fn push(&mut self, message: Message) {
        self.estimated_tokens += Self::estimate_message_tokens(&message);
        self.messages.push(message);
    }

    /// Update usage statistics.
    pub fn update_usage(&mut self, usage: Usage) {
        self.total_usage.input_tokens += usage.input_tokens;
        self.total_usage.output_tokens += usage.output_tokens;
        if let Some(cache_read) = usage.cache_read_input_tokens {
            self.total_usage.cache_read_input_tokens =
                Some(self.total_usage.cache_read_input_tokens.unwrap_or(0) + cache_read);
        }
    }

    /// Get all messages.
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get messages as mutable.
    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Get total usage.
    pub fn total_usage(&self) -> &Usage {
        &self.total_usage
    }

    /// Get estimated token count.
    pub fn estimated_tokens(&self) -> usize {
        self.estimated_tokens
    }

    /// Get the conversation summary if available.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Get the number of compactions performed.
    pub fn compactions(&self) -> usize {
        self.compactions
    }

    /// Check if context should be compacted.
    pub fn should_compact(&self, max_tokens: usize, threshold: f32) -> bool {
        self.messages.len() > KEEP_RECENT_MESSAGES
            && self.estimated_tokens as f32 > max_tokens as f32 * threshold
    }

    /// Compact the context by summarizing older messages.
    pub async fn compact(&mut self, client: &crate::Client) -> crate::Result<CompactResult> {
        if self.messages.len() <= KEEP_RECENT_MESSAGES {
            return Ok(CompactResult::NotNeeded);
        }

        let split_point = self.messages.len() - KEEP_RECENT_MESSAGES;
        let to_summarize = &self.messages[..split_point];
        let to_keep = self.messages[split_point..].to_vec();

        let summary_prompt = Self::format_for_summary(to_summarize);
        let summary = self.generate_summary(client, &summary_prompt).await?;

        let original_count = self.messages.len();
        let saved_tokens = self.estimated_tokens;

        self.messages.clear();
        self.messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::text(format!(
                "[Previous conversation summary]\n{}",
                summary
            ))],
        });
        for msg in to_keep {
            self.messages.push(msg);
        }

        self.estimated_tokens = self.recalculate_tokens();
        self.summary = Some(summary.clone());
        self.compactions += 1;

        Ok(CompactResult::Compacted {
            original_count,
            new_count: self.messages.len(),
            saved_tokens: saved_tokens.saturating_sub(self.estimated_tokens),
            summary,
        })
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.estimated_tokens = 0;
        self.summary = None;
    }

    /// Get message count.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn estimate_message_tokens(message: &Message) -> usize {
        message
            .content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text } => text.len() / CHARS_PER_TOKEN,
                _ => NON_TEXT_BLOCK_TOKENS,
            })
            .sum()
    }

    fn recalculate_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(Self::estimate_message_tokens)
            .sum()
    }

    fn format_for_summary(messages: &[Message]) -> String {
        let mut formatted = String::with_capacity(4096);
        formatted.push_str(
            "Summarize this conversation concisely. \
             Preserve key decisions, code changes, file paths, and important context:\n\n",
        );

        for msg in messages {
            let role = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
            };
            formatted.push_str(role);
            formatted.push_str(":\n");

            for block in &msg.content {
                if let Some(text) = block.as_text() {
                    if text.len() > 800 {
                        formatted.push_str(&text[..800]);
                        formatted.push_str("... [truncated]\n");
                    } else {
                        formatted.push_str(text);
                        formatted.push('\n');
                    }
                }
            }
            formatted.push('\n');
        }

        formatted
    }

    async fn generate_summary(
        &self,
        client: &crate::Client,
        prompt: &str,
    ) -> crate::Result<String> {
        use crate::client::messages::{CreateMessageRequest, MessagesClient};

        let request = CreateMessageRequest::new(SUMMARY_MODEL, vec![Message::user(prompt)])
            .with_max_tokens(2000);

        let response = MessagesClient::new(client).create(request).await?;
        Ok(response.text())
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
        for i in 0..10 {
            ctx.push(Message::user(format!("Message {}", i)));
        }
        ctx.estimated_tokens = 8000;
        assert!(ctx.should_compact(10000, 0.75));
        assert!(!ctx.should_compact(10000, 0.85));
    }

    #[test]
    fn test_should_compact_few_messages() {
        let mut ctx = ConversationContext::new();
        ctx.push(Message::user("Hello"));
        ctx.estimated_tokens = 8000;
        assert!(!ctx.should_compact(10000, 0.75));
    }

    #[test]
    fn test_format_for_summary() {
        let messages = vec![
            Message::user("Hello"),
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::text("Hi there!")],
            },
        ];
        let summary = ConversationContext::format_for_summary(&messages);
        assert!(summary.contains("User:"));
        assert!(summary.contains("Assistant:"));
        assert!(summary.contains("Hello"));
    }

    #[test]
    fn test_compactions_counter() {
        let ctx = ConversationContext::new();
        assert_eq!(ctx.compactions(), 0);
    }
}
