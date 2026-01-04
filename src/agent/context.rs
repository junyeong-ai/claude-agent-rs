//! Conversation context management.

use crate::types::{CacheControl, CompactResult, ContentBlock, Message, Role, Usage};

const CHARS_PER_TOKEN: usize = 4;
const NON_TEXT_BLOCK_TOKENS: usize = 25;
const MAX_MESSAGE_CACHE_BREAKPOINTS: usize = 3;
const MIN_TOKENS_FOR_CACHE: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageCacheStrategy {
    Disabled,
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Default)]
pub struct ConversationContext {
    messages: Vec<Message>,
    total_usage: Usage,
    estimated_tokens: usize,
    summary: Option<String>,
    compactions: usize,
    cache_strategy: MessageCacheStrategy,
    cache_breakpoints: Vec<usize>,
}

impl ConversationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cache_strategy(mut self, strategy: MessageCacheStrategy) -> Self {
        self.cache_strategy = strategy;
        self
    }

    pub fn set_cache_strategy(&mut self, strategy: MessageCacheStrategy) {
        self.cache_strategy = strategy;
    }

    pub fn cache_strategy(&self) -> MessageCacheStrategy {
        self.cache_strategy
    }

    pub fn push(&mut self, message: Message) {
        self.estimated_tokens += Self::estimate_message_tokens(&message);
        self.messages.push(message);

        if self.cache_strategy == MessageCacheStrategy::Auto {
            self.update_cache_breakpoints();
        }
    }

    pub fn update_usage(&mut self, usage: Usage) {
        self.total_usage.input_tokens += usage.input_tokens;
        self.total_usage.output_tokens += usage.output_tokens;
        if let Some(cache_read) = usage.cache_read_input_tokens {
            self.total_usage.cache_read_input_tokens =
                Some(self.total_usage.cache_read_input_tokens.unwrap_or(0) + cache_read);
        }
        if let Some(cache_creation) = usage.cache_creation_input_tokens {
            self.total_usage.cache_creation_input_tokens =
                Some(self.total_usage.cache_creation_input_tokens.unwrap_or(0) + cache_creation);
        }
        if let Some(ref server_usage) = usage.server_tool_use {
            let current = self
                .total_usage
                .server_tool_use
                .get_or_insert_with(Default::default);
            current.web_search_requests += server_usage.web_search_requests;
            current.web_fetch_requests += server_usage.web_fetch_requests;
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn messages_mut(&mut self) -> &mut Vec<Message> {
        &mut self.messages
    }

    /// Consume context and return owned messages.
    /// Use this at the end of execution to avoid cloning.
    pub fn into_messages(self) -> Vec<Message> {
        self.messages
    }

    /// Take messages, leaving an empty vec.
    /// Useful when you need to consume messages but keep the context.
    pub fn take_messages(&mut self) -> Vec<Message> {
        self.estimated_tokens = 0;
        self.cache_breakpoints.clear();
        std::mem::take(&mut self.messages)
    }

    pub fn total_usage(&self) -> &Usage {
        &self.total_usage
    }

    pub fn estimated_tokens(&self) -> usize {
        self.estimated_tokens
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    pub fn compactions(&self) -> usize {
        self.compactions
    }

    /// Set estimated tokens (for testing only).
    #[cfg(test)]
    pub fn set_estimated_tokens(&mut self, tokens: usize) {
        self.estimated_tokens = tokens;
    }

    pub fn should_compact(&self, max_tokens: usize, threshold: f32, keep_messages: usize) -> bool {
        self.messages.len() > keep_messages
            && self.estimated_tokens as f32 > max_tokens as f32 * threshold
    }

    pub async fn compact(
        &mut self,
        client: &crate::Client,
        keep_messages: usize,
    ) -> crate::Result<CompactResult> {
        if self.messages.len() <= keep_messages {
            return Ok(CompactResult::NotNeeded);
        }

        let split_point = self.messages.len() - keep_messages;
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

    pub fn clear(&mut self) {
        self.messages.clear();
        self.estimated_tokens = 0;
        self.summary = None;
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn cache_breakpoints(&self) -> &[usize] {
        &self.cache_breakpoints
    }

    pub fn set_manual_breakpoints(&mut self, breakpoints: Vec<usize>) {
        if self.cache_strategy == MessageCacheStrategy::Manual {
            self.cache_breakpoints = breakpoints
                .into_iter()
                .filter(|&i| i < self.messages.len())
                .take(MAX_MESSAGE_CACHE_BREAKPOINTS)
                .collect();
            self.apply_cache_breakpoints();
        }
    }

    fn update_cache_breakpoints(&mut self) {
        if self.messages.len() < 4 || self.estimated_tokens < MIN_TOKENS_FOR_CACHE {
            return;
        }

        self.clear_all_cache_control();

        let total_tokens = self.estimated_tokens;
        let mut breakpoints = Vec::with_capacity(MAX_MESSAGE_CACHE_BREAKPOINTS);

        let mut cumulative = 0;
        let target_interval = total_tokens / (MAX_MESSAGE_CACHE_BREAKPOINTS + 1);

        for (i, msg) in self.messages.iter().enumerate() {
            cumulative += Self::estimate_message_tokens(msg);

            if cumulative >= target_interval * (breakpoints.len() + 1)
                && breakpoints.len() < MAX_MESSAGE_CACHE_BREAKPOINTS
                && i < self.messages.len() - 2
            {
                breakpoints.push(i);
            }
        }

        self.cache_breakpoints = breakpoints;
        self.apply_cache_breakpoints();
    }

    fn apply_cache_breakpoints(&mut self) {
        for &idx in &self.cache_breakpoints {
            if let Some(msg) = self.messages.get_mut(idx) {
                msg.set_cache_on_last_block(CacheControl::ephemeral());
            }
        }
    }

    fn clear_all_cache_control(&mut self) {
        for msg in &mut self.messages {
            msg.clear_cache_control();
        }
    }

    pub fn messages_with_cache(&self) -> Vec<Message> {
        self.messages.clone()
    }

    fn estimate_message_tokens(message: &Message) -> usize {
        message
            .content
            .iter()
            .map(|block| match block {
                ContentBlock::Text { text, .. } => text.len() / CHARS_PER_TOKEN,
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
        use crate::client::ModelType;
        use crate::client::messages::CreateMessageRequest;

        let model = client.adapter().model(ModelType::Small).to_string();
        let request =
            CreateMessageRequest::new(&model, vec![Message::user(prompt)]).with_max_tokens(2000);

        let response = client.send(request).await?;
        Ok(response.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_push() {
        let mut history = ConversationContext::new();
        history.push(Message::user("Hello"));
        assert_eq!(history.len(), 1);
        assert!(history.estimated_tokens() > 0);
    }

    #[test]
    fn test_should_compact() {
        let mut history = ConversationContext::new();
        for i in 0..10 {
            history.push(Message::user(format!("Message {}", i)));
        }
        history.estimated_tokens = 8000;
        assert!(history.should_compact(10000, 0.75, 4));
        assert!(!history.should_compact(10000, 0.85, 4));
    }

    #[test]
    fn test_should_compact_few_messages() {
        let mut history = ConversationContext::new();
        history.push(Message::user("Hello"));
        history.estimated_tokens = 8000;
        assert!(!history.should_compact(10000, 0.75, 4));
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
        let history = ConversationContext::new();
        assert_eq!(history.compactions(), 0);
    }

    #[test]
    fn test_cache_strategy_default() {
        let ctx = ConversationContext::new();
        assert_eq!(ctx.cache_strategy(), MessageCacheStrategy::Auto);
    }

    #[test]
    fn test_cache_strategy_disabled() {
        let mut ctx =
            ConversationContext::new().with_cache_strategy(MessageCacheStrategy::Disabled);

        for i in 0..10 {
            ctx.push(Message::user(format!(
                "Message {} with lots of content to ensure tokens",
                i
            )));
        }
        ctx.estimated_tokens = 5000;

        assert!(ctx.cache_breakpoints().is_empty());
    }

    #[test]
    fn test_auto_cache_breakpoints() {
        let mut ctx = ConversationContext::new();

        let long_content = "x".repeat(500);
        for i in 0..20 {
            ctx.push(Message::user(format!("Message {} {}", i, long_content)));
        }

        assert!(ctx.estimated_tokens() >= MIN_TOKENS_FOR_CACHE);
        assert!(!ctx.cache_breakpoints().is_empty());
        assert!(ctx.cache_breakpoints().len() <= MAX_MESSAGE_CACHE_BREAKPOINTS);
    }

    #[test]
    fn test_manual_cache_breakpoints() {
        let mut ctx = ConversationContext::new().with_cache_strategy(MessageCacheStrategy::Manual);

        for i in 0..10 {
            ctx.push(Message::user(format!("Message {}", i)));
        }

        ctx.set_manual_breakpoints(vec![2, 5, 8]);
        assert_eq!(ctx.cache_breakpoints(), &[2, 5, 8]);
    }

    #[test]
    fn test_cache_breakpoints_not_set_for_small_context() {
        let mut ctx = ConversationContext::new();
        ctx.push(Message::user("Short message"));
        ctx.push(Message::user("Another short one"));

        assert!(ctx.cache_breakpoints().is_empty());
    }
}
