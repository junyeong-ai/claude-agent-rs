//! SSE streaming support for the Anthropic Messages API.

use bytes::Bytes;
use futures::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::Result;
use crate::types::StreamEvent;

/// Item emitted by the stream parser.
#[derive(Debug, Clone)]
pub enum StreamItem {
    /// A parsed event.
    Event(StreamEvent),
    /// Raw text delta (convenience extraction).
    Text(String),
}

pin_project! {
    /// Parser for SSE streams from the Anthropic API.
    ///
    /// Handles the SSE format:
    /// ```text
    /// event: message_start
    /// data: {"type":"message_start",...}
    ///
    /// event: content_block_delta
    /// data: {"type":"content_block_delta",...}
    /// ```
    pub struct StreamParser<S> {
        #[pin]
        inner: S,
        buffer: String,
    }
}

impl<S> StreamParser<S>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>>,
{
    /// Create a new stream parser.
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            buffer: String::new(),
        }
    }

    /// Extract JSON data from an SSE event block.
    ///
    /// SSE format can be:
    /// - `data: {json}` (simple)
    /// - `event: type\ndata: {json}` (with event type)
    fn extract_json_data(event_block: &str) -> Option<&str> {
        for line in event_block.lines() {
            let line = line.trim();
            if let Some(json_str) = line.strip_prefix("data: ") {
                let json_str = json_str.trim();
                // Skip [DONE] marker and ping events
                if json_str == "[DONE]"
                    || json_str.contains("\"type\": \"ping\"")
                    || json_str.contains("\"type\":\"ping\"")
                {
                    return None;
                }
                if !json_str.is_empty() {
                    return Some(json_str);
                }
            }
        }
        None
    }

    /// Parse a single SSE event from the event block.
    fn parse_event(event_block: &str) -> Option<StreamEvent> {
        // Skip empty blocks and comments
        let trimmed = event_block.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            return None;
        }

        // Extract JSON data
        let json_str = Self::extract_json_data(event_block)?;

        // Parse JSON
        match serde_json::from_str::<StreamEvent>(json_str) {
            Ok(event) => Some(event),
            Err(e) => {
                tracing::warn!("Failed to parse stream event: {} - data: {}", e, json_str);
                None
            }
        }
    }
}

impl<S> Stream for StreamParser<S>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>>,
{
    type Item = Result<StreamItem>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        loop {
            // Try to parse complete events from buffer (events are separated by \n\n)
            if let Some(pos) = this.buffer.find("\n\n") {
                let event_block = this.buffer[..pos].to_string();
                *this.buffer = this.buffer[pos + 2..].to_string();

                if let Some(event) = Self::parse_event(&event_block) {
                    // Extract text delta for convenience
                    let item = match &event {
                        StreamEvent::ContentBlockDelta {
                            delta: crate::types::ContentDelta::TextDelta { text },
                            ..
                        } => StreamItem::Text(text.clone()),
                        _ => StreamItem::Event(event),
                    };
                    return Poll::Ready(Some(Ok(item)));
                }
                // Continue to try parsing more events
                continue;
            }

            // Need more data from the underlying stream
            match this.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => match std::str::from_utf8(&bytes) {
                    Ok(s) => this.buffer.push_str(s),
                    Err(e) => {
                        return Poll::Ready(Some(Err(crate::Error::Config(format!(
                            "Invalid UTF-8 in stream: {}",
                            e
                        )))));
                    }
                },
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(crate::Error::Network(e))));
                }
                Poll::Ready(None) => {
                    // Stream ended, try to parse any remaining data
                    if !this.buffer.is_empty() {
                        let remaining = std::mem::take(this.buffer);
                        if let Some(event) = Self::parse_event(&remaining) {
                            return Poll::Ready(Some(Ok(StreamItem::Event(event))));
                        }
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type EmptyStream = futures::stream::Empty<std::result::Result<Bytes, reqwest::Error>>;

    #[test]
    fn test_parse_simple_data() {
        let data = r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event = StreamParser::<EmptyStream>::parse_event(data);
        assert!(event.is_some());
    }

    #[test]
    fn test_parse_event_with_type() {
        let data = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}";
        let event = StreamParser::<EmptyStream>::parse_event(data);
        assert!(event.is_some());
    }

    #[test]
    fn test_parse_message_start() {
        let data = r#"event: message_start
data: {"type":"message_start","message":{"model":"claude-sonnet-4-5","id":"msg_123","type":"message","role":"assistant","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":1}}}"#;
        let event = StreamParser::<EmptyStream>::parse_event(data);
        assert!(event.is_some());
        if let Some(StreamEvent::MessageStart { .. }) = event {
            // OK
        } else {
            panic!("Expected MessageStart event");
        }
    }

    #[test]
    fn test_skip_done_marker() {
        let data = "data: [DONE]";
        let event = StreamParser::<EmptyStream>::parse_event(data);
        assert!(event.is_none());
    }

    #[test]
    fn test_skip_ping_event() {
        let data = "event: ping\ndata: {\"type\": \"ping\"}";
        let event = StreamParser::<EmptyStream>::parse_event(data);
        assert!(event.is_none());
    }

    #[test]
    fn test_skip_empty_block() {
        let event = StreamParser::<EmptyStream>::parse_event("");
        assert!(event.is_none());

        let event = StreamParser::<EmptyStream>::parse_event("   \n  ");
        assert!(event.is_none());
    }

    #[test]
    fn test_skip_comment() {
        let data = ": this is a comment";
        let event = StreamParser::<EmptyStream>::parse_event(data);
        assert!(event.is_none());
    }

    #[test]
    fn test_extract_json_data() {
        // Simple data line
        let json = StreamParser::<EmptyStream>::extract_json_data("data: {\"foo\":\"bar\"}");
        assert_eq!(json, Some("{\"foo\":\"bar\"}"));

        // With event type
        let json =
            StreamParser::<EmptyStream>::extract_json_data("event: test\ndata: {\"foo\":\"bar\"}");
        assert_eq!(json, Some("{\"foo\":\"bar\"}"));

        // Skip [DONE]
        let json = StreamParser::<EmptyStream>::extract_json_data("data: [DONE]");
        assert!(json.is_none());

        // Skip ping
        let json = StreamParser::<EmptyStream>::extract_json_data(
            "event: ping\ndata: {\"type\": \"ping\"}",
        );
        assert!(json.is_none());
    }
}
