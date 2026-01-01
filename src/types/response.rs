//! API response types.

use serde::{Deserialize, Serialize};

use super::ContentBlock;

/// Accumulated token usage for session tracking (u64 for overflow safety).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens consumed
    pub input_tokens: u64,
    /// Output tokens generated
    pub output_tokens: u64,
    /// Tokens read from cache
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    /// Tokens written to cache
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
}

impl TokenUsage {
    /// Total tokens (input + output)
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// Accumulate from another TokenUsage
    pub fn add(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_input_tokens += other.cache_read_input_tokens;
        self.cache_creation_input_tokens += other.cache_creation_input_tokens;
    }

    /// Accumulate from API Usage
    pub fn add_usage(&mut self, usage: &Usage) {
        self.input_tokens += usage.input_tokens as u64;
        self.output_tokens += usage.output_tokens as u64;
        self.cache_read_input_tokens += usage.cache_read_input_tokens.unwrap_or(0) as u64;
        self.cache_creation_input_tokens += usage.cache_creation_input_tokens.unwrap_or(0) as u64;
    }

    /// Cache hit rate (0.0 to 1.0)
    pub fn cache_hit_rate(&self) -> f64 {
        if self.input_tokens == 0 {
            return 0.0;
        }
        self.cache_read_input_tokens as f64 / self.input_tokens as f64
    }
}

impl From<&Usage> for TokenUsage {
    fn from(usage: &Usage) -> Self {
        Self {
            input_tokens: usage.input_tokens as u64,
            output_tokens: usage.output_tokens as u64,
            cache_read_input_tokens: usage.cache_read_input_tokens.unwrap_or(0) as u64,
            cache_creation_input_tokens: usage.cache_creation_input_tokens.unwrap_or(0) as u64,
        }
    }
}

/// Response from the Messages API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    /// Unique identifier for this response
    pub id: String,
    /// Type of response (always "message")
    #[serde(rename = "type")]
    pub response_type: String,
    /// Role (always "assistant")
    pub role: String,
    /// Content blocks in the response
    pub content: Vec<ContentBlock>,
    /// Model that generated the response
    pub model: String,
    /// Reason for stopping
    pub stop_reason: Option<StopReason>,
    /// Sequence that caused stop (if stop_reason is "stop_sequence")
    pub stop_sequence: Option<String>,
    /// Token usage information
    pub usage: Usage,
}

/// Reason why Claude stopped generating
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of response
    EndTurn,
    /// Max tokens reached
    MaxTokens,
    /// Stop sequence encountered
    StopSequence,
    /// Tool use requested
    ToolUse,
}

/// Token usage information
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens consumed
    pub input_tokens: u32,
    /// Output tokens generated
    pub output_tokens: u32,
    /// Tokens read from cache (prompt caching)
    #[serde(default)]
    pub cache_read_input_tokens: Option<u32>,
    /// Tokens written to cache (prompt caching)
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u32>,
}

impl Usage {
    /// Total tokens used (input + output)
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }

    /// Estimated cost in USD (rough approximation)
    pub fn estimated_cost(&self, model: &str) -> f64 {
        let (input_rate, output_rate) = match model {
            m if m.contains("opus") => (15.0, 75.0),  // per 1M tokens
            m if m.contains("sonnet") => (3.0, 15.0), // per 1M tokens
            m if m.contains("haiku") => (0.25, 1.25), // per 1M tokens
            _ => (3.0, 15.0),                         // default to Sonnet
        };

        let input_cost = (self.input_tokens as f64 / 1_000_000.0) * input_rate;
        let output_cost = (self.output_tokens as f64 / 1_000_000.0) * output_rate;

        input_cost + output_cost
    }
}

impl ApiResponse {
    /// Get the text content of the response (concatenated)
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Check if the response wants to use tools
    pub fn wants_tool_use(&self) -> bool {
        self.stop_reason == Some(StopReason::ToolUse)
    }

    /// Extract all tool use blocks
    pub fn tool_uses(&self) -> Vec<&super::ToolUseBlock> {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse(tool_use) => Some(tool_use),
                _ => None,
            })
            .collect()
    }
}

/// Streaming event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Start of message
    MessageStart {
        /// The message being started
        message: MessageStartData,
    },
    /// Start of a content block
    ContentBlockStart {
        /// Index of the block
        index: usize,
        /// The content block
        content_block: ContentBlock,
    },
    /// Delta update for a content block
    ContentBlockDelta {
        /// Index of the block
        index: usize,
        /// The delta update
        delta: ContentDelta,
    },
    /// End of a content block
    ContentBlockStop {
        /// Index of the block
        index: usize,
    },
    /// Delta update for message
    MessageDelta {
        /// Delta update
        delta: MessageDeltaData,
        /// Updated usage
        usage: Usage,
    },
    /// End of message
    MessageStop,
    /// Ping event (keep-alive)
    Ping,
    /// Error event
    Error {
        /// Error details
        error: StreamError,
    },
}

/// Data for message_start event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStartData {
    /// Message ID
    pub id: String,
    /// Message type
    #[serde(rename = "type")]
    pub message_type: String,
    /// Role
    pub role: String,
    /// Model
    pub model: String,
    /// Initial usage
    pub usage: Usage,
}

/// Delta update for content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    /// Text delta
    TextDelta {
        /// The text fragment
        text: String,
    },
    /// Input JSON delta (for tool use)
    InputJsonDelta {
        /// Partial JSON string
        partial_json: String,
    },
}

/// Delta update for message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaData {
    /// Stop reason if completed
    pub stop_reason: Option<StopReason>,
    /// Stop sequence if applicable
    pub stop_sequence: Option<String>,
}

/// Error in stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    /// Error type.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Error message.
    pub message: String,
}

/// Result of a context compaction operation.
#[derive(Debug, Clone)]
pub enum CompactResult {
    /// Compaction was not needed (too few messages or below threshold).
    NotNeeded,
    /// Compaction was performed successfully.
    Compacted {
        /// Original message count before compaction.
        original_count: usize,
        /// New message count after compaction.
        new_count: usize,
        /// Estimated tokens saved.
        saved_tokens: usize,
        /// Generated summary text.
        summary: String,
    },
    /// Compaction was skipped.
    Skipped {
        /// Reason for skipping.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_total() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            ..Default::default()
        };
        assert_eq!(usage.total(), 150);
    }

    #[test]
    fn test_usage_cost() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            ..Default::default()
        };
        // Sonnet: $3 input + $15 output = $18
        let cost = usage.estimated_cost("claude-sonnet-4-5");
        assert!((cost - 18.0).abs() < 0.01);
    }
}
