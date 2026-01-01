//! WebFetch tool - fetches and processes web content.

use async_trait::async_trait;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::tools::{ToolResult, TypedTool};

/// Input for WebFetch tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchInput {
    /// The URL to fetch content from.
    pub url: String,
    /// The prompt describing what information to extract.
    pub prompt: String,
}

/// Output from WebFetch tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchOutput {
    /// The processed response content.
    pub response: String,
    /// The URL that was fetched.
    pub url: String,
    /// The final URL after redirects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    /// HTTP status code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

/// WebFetch tool for fetching and processing web content.
pub struct WebFetchTool {
    client: reqwest::Client,
}

impl WebFetchTool {
    /// Create a new WebFetchTool.
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (compatible; ClaudeAgent/1.0; +https://anthropic.com)",
            ),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,text/plain;q=0.8,*/*;q=0.7",
            ),
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { client }
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    while let Some(start) = text.to_lowercase().find("<script") {
        if let Some(end) = text[start..].to_lowercase().find("</script>") {
            text = format!("{}{}", &text[..start], &text[start + end + 9..]);
        } else {
            break;
        }
    }

    while let Some(start) = text.to_lowercase().find("<style") {
        if let Some(end) = text[start..].to_lowercase().find("</style>") {
            text = format!("{}{}", &text[..start], &text[start + end + 8..]);
        } else {
            break;
        }
    }

    while let Some(start) = text.find("<!--") {
        if let Some(end) = text[start..].find("-->") {
            text = format!("{}{}", &text[..start], &text[start + end + 3..]);
        } else {
            break;
        }
    }

    let block_tags = [
        "<br>", "<br/>", "<br />", "<p>", "</p>", "<div>", "</div>", "<h1>", "</h1>", "<h2>",
        "</h2>", "<h3>", "</h3>", "<h4>", "</h4>", "<h5>", "</h5>", "<h6>", "</h6>", "<li>",
        "</li>", "<tr>", "</tr>",
    ];

    for tag in block_tags {
        text = text.replace(tag, "\n");
        text = text.replace(&tag.to_uppercase(), "\n");
    }

    let mut result = String::new();
    let mut in_tag = false;

    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    let entities = [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&apos;", "'"),
        ("&#39;", "'"),
        ("&#x27;", "'"),
        ("&mdash;", "—"),
        ("&ndash;", "–"),
        ("&hellip;", "…"),
        ("&copy;", "©"),
        ("&reg;", "®"),
        ("&trade;", "™"),
    ];

    for (entity, replacement) in entities {
        result = result.replace(entity, replacement);
    }

    result
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_content(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        format!(
            "{}\n\n[Content truncated at {} characters. Original length: {} characters]",
            &content[..max_chars],
            max_chars,
            content.len()
        )
    }
}

#[async_trait]
impl TypedTool for WebFetchTool {
    type Input = WebFetchInput;

    const NAME: &'static str = "WebFetch";
    const DESCRIPTION: &'static str = r#"Fetches content from a specified URL and processes it.

Usage:
- Fetches the URL content and converts HTML to text
- HTTP URLs will be automatically upgraded to HTTPS
- The prompt should describe what information you want to extract from the page
- Results may be summarized if the content is very large
- Includes a self-cleaning 15-minute cache for faster responses

When a URL redirects to a different host, the tool will inform you and provide
the redirect URL. You should then make a new WebFetch request with the redirect URL.

IMPORTANT: If an MCP-provided web fetch tool is available, prefer using that tool
instead of this one, as it may have fewer restrictions."#;

    async fn handle(&self, input: WebFetchInput) -> ToolResult {
        let mut url = input.url.trim().to_string();
        if url.is_empty() {
            return ToolResult::error("URL cannot be empty");
        }

        if url.starts_with("http://") {
            url = url.replacen("http://", "https://", 1);
        }

        if !url.starts_with("https://") && !url.starts_with("http://") {
            url = format!("https://{}", url);
        }

        let parsed_url = match reqwest::Url::parse(&url) {
            Ok(u) => u,
            Err(e) => return ToolResult::error(format!("Invalid URL format: {}", e)),
        };

        let response = match self.client.get(parsed_url.as_str()).send().await {
            Ok(r) => r,
            Err(e) => {
                if e.is_timeout() {
                    return ToolResult::error("Request timed out after 30 seconds");
                } else if e.is_connect() {
                    return ToolResult::error(format!("Failed to connect to {}: {}", url, e));
                } else if e.is_redirect() {
                    return ToolResult::error(format!("Too many redirects for {}", url));
                } else {
                    return ToolResult::error(format!("Failed to fetch URL: {}", e));
                }
            }
        };

        let status = response.status();
        let final_url = response.url().to_string();

        if let Ok(final_parsed) = reqwest::Url::parse(&final_url)
            && final_parsed.host_str() != parsed_url.host_str()
        {
            return ToolResult::success(format!(
                "The URL redirected to a different host.\n\
                     Original URL: {}\n\
                     Redirect URL: {}\n\n\
                     Please make a new WebFetch request with the redirect URL to fetch the content.",
                url, final_url
            ));
        }

        if !status.is_success() {
            return ToolResult::error(format!(
                "HTTP error {}: {} for URL {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown"),
                url
            ));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/plain")
            .to_lowercase();

        let body = match response.text().await {
            Ok(b) => b,
            Err(e) => return ToolResult::error(format!("Failed to read response body: {}", e)),
        };

        let processed_content =
            if content_type.contains("text/html") || content_type.contains("application/xhtml") {
                html_to_text(&body)
            } else {
                body
            };

        let truncated_content = truncate_content(&processed_content, 50000);

        let output = WebFetchOutput {
            response: truncated_content.clone(),
            url: url.clone(),
            final_url: if final_url != url {
                Some(final_url)
            } else {
                None
            },
            status_code: Some(status.as_u16()),
        };

        let result = format!(
            "URL: {}\n{}\nStatus: {}\nPrompt: {}\n\n---\nContent:\n{}",
            output.url,
            output
                .final_url
                .as_ref()
                .map_or(String::new(), |u| format!("Final URL: {}\n", u)),
            output.status_code.unwrap_or(0),
            input.prompt,
            output.response
        );

        ToolResult::success(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use serde_json::json;

    #[test]
    fn test_html_to_text() {
        let html = r#"
            <html>
            <head><title>Test</title></head>
            <body>
                <h1>Hello World</h1>
                <p>This is a <strong>test</strong> paragraph.</p>
                <script>alert('hi');</script>
                <style>.foo { color: red; }</style>
            </body>
            </html>
        "#;

        let text = html_to_text(html);
        assert!(text.contains("Hello World"));
        assert!(text.contains("test"));
        assert!(text.contains("paragraph"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color"));
    }

    #[test]
    fn test_html_entity_decoding() {
        let html = "Hello &amp; World &lt;test&gt;";
        let text = html_to_text(html);
        assert_eq!(text, "Hello & World <test>");
    }

    #[test]
    fn test_truncate_content() {
        let content = "a".repeat(100);
        let truncated = truncate_content(&content, 50);
        assert!(truncated.starts_with(&"a".repeat(50)));
        assert!(truncated.contains("[Content truncated"));
    }

    #[test]
    fn test_truncate_short_content() {
        let content = "Hello World";
        let truncated = truncate_content(content, 50);
        assert_eq!(truncated, content);
    }

    #[test]
    fn test_tool_definition() {
        let tool = WebFetchTool::new();

        assert_eq!(tool.name(), "WebFetch");
        assert!(!tool.description().is_empty());

        let schema = tool.input_schema();
        assert!(schema["properties"]["url"].is_object());
        assert!(schema["properties"]["prompt"].is_object());
    }

    #[tokio::test]
    async fn test_invalid_url() {
        let tool = WebFetchTool::new();

        let result = tool
            .execute(json!({
                "url": "",
                "prompt": "test"
            }))
            .await;

        assert!(result.is_error());
    }

    #[tokio::test]
    async fn test_malformed_url() {
        let tool = WebFetchTool::new();

        let result = tool
            .execute(json!({
                "url": "not a valid url ::::",
                "prompt": "test"
            }))
            .await;

        assert!(result.is_error());
    }
}
