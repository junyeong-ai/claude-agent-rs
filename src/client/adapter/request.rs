//! Shared request building utilities for cloud adapters.

#![cfg_attr(
    not(any(feature = "aws", feature = "gcp", feature = "azure")),
    allow(dead_code)
)]

use serde_json::{Value, json};

use crate::client::messages::CreateMessageRequest;

pub fn build_messages_body(
    request: &CreateMessageRequest,
    anthropic_version: Option<&str>,
    thinking_budget: Option<u32>,
) -> Value {
    let mut body = json!({
        "max_tokens": request.max_tokens,
        "messages": request.messages,
    });

    if let Some(version) = anthropic_version {
        body["anthropic_version"] = json!(version);
    }
    if !request.model.is_empty() {
        body["model"] = json!(request.model);
    }
    if let Some(ref system) = request.system {
        body["system"] = json!(system);
    }
    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }
    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }
    if let Some(top_k) = request.top_k {
        body["top_k"] = json!(top_k);
    }
    if let Some(ref stop) = request.stop_sequences {
        body["stop_sequences"] = json!(stop);
    }
    if let Some(stream) = request.stream {
        body["stream"] = json!(stream);
    }
    if let Some(ref tools) = request.tools {
        body["tools"] = json!(tools);
    }
    if let Some(ref tool_choice) = request.tool_choice {
        body["tool_choice"] = json!(tool_choice);
    }
    if let Some(ref thinking) = request.thinking {
        body["thinking"] = json!(thinking);
    } else if let Some(budget) = thinking_budget {
        body["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": budget
        });
    }

    body
}

pub fn add_beta_features(body: &mut Value, features: &[&str]) {
    if features.is_empty() {
        return;
    }
    if let Some(obj) = body.as_object_mut() {
        obj.insert("anthropic_beta".to_string(), json!(features));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;

    fn sample_request() -> CreateMessageRequest {
        CreateMessageRequest::new("claude-sonnet-4-20250514", vec![Message::user("Hello")])
    }

    #[test]
    fn test_build_messages_body_basic() {
        let request = sample_request();
        let body = build_messages_body(&request, Some("bedrock-2023-05-31"), None);

        assert_eq!(body["anthropic_version"], "bedrock-2023-05-31");
        assert_eq!(body["max_tokens"], 8192); // Default from CreateMessageRequest::new
        assert!(!body["messages"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_build_messages_body_with_thinking() {
        let request = sample_request();
        let body = build_messages_body(&request, None, Some(5000));

        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 5000);
    }

    #[test]
    fn test_add_beta_features() {
        let request = sample_request();
        let mut body = build_messages_body(&request, None, None);

        add_beta_features(&mut body, &["context-1m-2025-08-07"]);

        let beta = body["anthropic_beta"].as_array().unwrap();
        assert_eq!(beta[0], "context-1m-2025-08-07");
    }

    #[test]
    fn test_build_messages_body_with_optional_fields() {
        let mut request = sample_request();
        request.temperature = Some(0.7);
        request.top_p = Some(0.9);
        request.stop_sequences = Some(vec!["END".into()]);

        let body = build_messages_body(&request, None, None);

        // Use as_f64() for float comparison due to f32 -> JSON precision
        assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 0.01);
        assert!((body["top_p"].as_f64().unwrap() - 0.9).abs() < 0.01);
        assert_eq!(body["stop_sequences"][0], "END");
    }
}
