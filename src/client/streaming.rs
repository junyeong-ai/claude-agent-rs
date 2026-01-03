//! SSE streaming support for the Anthropic Messages API.

use bytes::Bytes;
use futures::Stream;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

use super::recovery::StreamRecoveryState;
use crate::Result;
use crate::types::{Citation, ContentDelta, StreamEvent};

#[derive(Debug, Clone)]
pub enum StreamItem {
    Event(StreamEvent),
    Text(String),
    Thinking(String),
    Citation(Citation),
}

pin_project! {
    pub struct StreamParser<S> {
        #[pin]
        inner: S,
        buffer: Vec<u8>,
        pos: usize,
    }
}

impl<S> StreamParser<S>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>>,
{
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(4096),
            pos: 0,
        }
    }

    #[inline]
    fn find_delimiter(buf: &[u8]) -> Option<usize> {
        buf.windows(2).position(|w| w == b"\n\n")
    }

    fn extract_json_data(event_block: &str) -> Option<&str> {
        for line in event_block.lines() {
            let line = line.trim();
            if let Some(json_str) = line.strip_prefix("data: ") {
                let json_str = json_str.trim();
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

    fn parse_event(event_block: &str) -> Option<StreamEvent> {
        let trimmed = event_block.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            return None;
        }
        let json_str = Self::extract_json_data(event_block)?;
        serde_json::from_str::<StreamEvent>(json_str)
            .inspect_err(|e| {
                tracing::warn!("Failed to parse stream event: {} - data: {}", e, json_str)
            })
            .ok()
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
            let search_slice = &this.buffer[*this.pos..];
            if let Some(rel_pos) = Self::find_delimiter(search_slice) {
                let start_pos = *this.pos;
                let end_pos = start_pos + rel_pos;
                let event_block = match std::str::from_utf8(&this.buffer[start_pos..end_pos]) {
                    Ok(s) => s,
                    Err(e) => {
                        return Poll::Ready(Some(Err(crate::Error::Config(format!(
                            "Invalid UTF-8 in event: {}",
                            e
                        )))));
                    }
                };

                let event = Self::parse_event(event_block);
                *this.pos = end_pos + 2;

                if this.buffer.len() > 8192 && *this.pos > this.buffer.len() / 2 {
                    this.buffer.drain(..*this.pos);
                    *this.pos = 0;
                }

                if let Some(event) = event {
                    let item = match &event {
                        StreamEvent::ContentBlockDelta {
                            delta: ContentDelta::TextDelta { text },
                            ..
                        } => StreamItem::Text(text.clone()),
                        StreamEvent::ContentBlockDelta {
                            delta: ContentDelta::ThinkingDelta { thinking },
                            ..
                        } => StreamItem::Thinking(thinking.clone()),
                        StreamEvent::ContentBlockDelta {
                            delta: ContentDelta::CitationsDelta { citation },
                            ..
                        } => StreamItem::Citation(citation.clone()),
                        _ => StreamItem::Event(event),
                    };
                    return Poll::Ready(Some(Ok(item)));
                }
                continue;
            }

            match this.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    if *this.pos > 0 && this.buffer.len() + bytes.len() > 16384 {
                        this.buffer.drain(..*this.pos);
                        *this.pos = 0;
                    }
                    this.buffer.extend_from_slice(&bytes);
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(crate::Error::Network(e))));
                }
                Poll::Ready(None) => {
                    if *this.pos < this.buffer.len() {
                        let remaining = match std::str::from_utf8(&this.buffer[*this.pos..]) {
                            Ok(s) => s,
                            Err(_) => return Poll::Ready(None),
                        };
                        if let Some(event) = Self::parse_event(remaining) {
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

pin_project! {
    pub struct RecoverableStream<S> {
        #[pin]
        inner: StreamParser<S>,
        recovery: StreamRecoveryState,
        current_block_type: Option<BlockType>,
    }
}

#[derive(Debug, Clone, Copy)]
enum BlockType {
    Text,
    Thinking,
    ToolUse,
}

impl<S> RecoverableStream<S>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>>,
{
    pub fn new(inner: S) -> Self {
        Self {
            inner: StreamParser::new(inner),
            recovery: StreamRecoveryState::new(),
            current_block_type: None,
        }
    }

    pub fn recovery_state(&self) -> &StreamRecoveryState {
        &self.recovery
    }

    pub fn take_recovery_state(self) -> StreamRecoveryState {
        self.recovery
    }
}

impl<S> Stream for RecoverableStream<S>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>>,
{
    type Item = Result<StreamItem>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(item))) => {
                match &item {
                    StreamItem::Text(text) => {
                        *this.current_block_type = Some(BlockType::Text);
                        this.recovery.append_text(text);
                    }
                    StreamItem::Thinking(thinking) => {
                        *this.current_block_type = Some(BlockType::Thinking);
                        this.recovery.append_thinking(thinking);
                    }
                    StreamItem::Event(event) => match event {
                        StreamEvent::ContentBlockStart {
                            content_block: crate::types::ContentBlock::ToolUse(tu),
                            ..
                        } => {
                            *this.current_block_type = Some(BlockType::ToolUse);
                            this.recovery.start_tool_use(tu.id.clone(), tu.name.clone());
                        }
                        StreamEvent::ContentBlockDelta {
                            delta: ContentDelta::InputJsonDelta { partial_json },
                            ..
                        } => {
                            this.recovery.append_tool_json(partial_json);
                        }
                        StreamEvent::ContentBlockDelta {
                            delta: ContentDelta::SignatureDelta { signature },
                            ..
                        } => {
                            this.recovery.append_signature(signature);
                        }
                        StreamEvent::ContentBlockStop { .. } => {
                            match this.current_block_type.take() {
                                Some(BlockType::Text) => this.recovery.complete_text_block(),
                                Some(BlockType::Thinking) => {
                                    this.recovery.complete_thinking_block()
                                }
                                Some(BlockType::ToolUse) => this.recovery.complete_tool_use_block(),
                                None => {}
                            }
                        }
                        _ => {}
                    },
                    StreamItem::Citation(_) => {}
                }
                Poll::Ready(Some(Ok(item)))
            }
            other => other,
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
        assert!(matches!(event, Some(StreamEvent::MessageStart { .. })));
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
        assert!(StreamParser::<EmptyStream>::parse_event("").is_none());
        assert!(StreamParser::<EmptyStream>::parse_event("   \n  ").is_none());
    }

    #[test]
    fn test_skip_comment() {
        let data = ": this is a comment";
        let event = StreamParser::<EmptyStream>::parse_event(data);
        assert!(event.is_none());
    }

    #[test]
    fn test_extract_json_data() {
        let json = StreamParser::<EmptyStream>::extract_json_data("data: {\"foo\":\"bar\"}");
        assert_eq!(json, Some("{\"foo\":\"bar\"}"));

        let json =
            StreamParser::<EmptyStream>::extract_json_data("event: test\ndata: {\"foo\":\"bar\"}");
        assert_eq!(json, Some("{\"foo\":\"bar\"}"));

        let json = StreamParser::<EmptyStream>::extract_json_data("data: [DONE]");
        assert!(json.is_none());

        let json = StreamParser::<EmptyStream>::extract_json_data(
            "event: ping\ndata: {\"type\": \"ping\"}",
        );
        assert!(json.is_none());
    }
}
