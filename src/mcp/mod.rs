//! MCP (Model Context Protocol) server integration.

pub mod client;
pub mod manager;
pub mod resources;
pub mod toolset;

pub use client::McpClient;
pub use manager::McpManager;
pub use resources::{ResourceManager, ResourceQuery};
pub use toolset::{McpToolset, McpToolsetRegistry, ToolLoadConfig};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const MCP_TOOL_PREFIX: &str = "mcp__";

/// Default timeout for MCP connections
pub const MCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Default timeout for MCP tool calls
pub const MCP_CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Default timeout for MCP resource reads
pub const MCP_RESOURCE_TIMEOUT: Duration = Duration::from_secs(30);

/// MCP server configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    /// stdio transport - communicates with server via stdin/stdout
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    /// Server-Sent Events transport (requires rmcp SSE support)
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// Reconnection policy with exponential backoff and jitter
#[derive(Clone, Debug)]
pub struct ReconnectPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_factor: f64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            jitter_factor: 0.3,
        }
    }
}

impl ReconnectPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> std::time::Duration {
        let base = self.base_delay_ms * 2u64.pow(attempt.min(10));
        let jitter = (base as f64 * self.jitter_factor * rand_factor()) as u64;
        std::time::Duration::from_millis((base + jitter).min(self.max_delay_ms))
    }
}

fn rand_factor() -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let mut hasher = DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    std::thread::current().id().hash(&mut hasher);
    std::process::id().hash(&mut hasher);

    let hash = hasher.finish();
    (hash % 10000) as f64 / 10000.0
}

/// Parse MCP qualified name (mcp__server_tool) into (server, tool)
pub fn parse_mcp_name(name: &str) -> Option<(&str, &str)> {
    name.strip_prefix(MCP_TOOL_PREFIX)?.split_once('_')
}

/// Create MCP qualified name from server and tool names
pub fn make_mcp_name(server: &str, tool: &str) -> String {
    format!("{}{server}_{tool}", MCP_TOOL_PREFIX)
}

/// Check if a name matches MCP naming pattern
pub fn is_mcp_name(name: &str) -> bool {
    name.starts_with(MCP_TOOL_PREFIX)
}

/// MCP connection status
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum McpConnectionStatus {
    /// Attempting to connect
    #[default]
    Connecting,
    /// Successfully connected
    Connected,
    /// Disconnected
    Disconnected,
    /// Connection failed
    Failed,
    /// Server requires authentication
    NeedsAuth,
}

/// MCP server information returned during initialization
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Protocol version (e.g., "2025-06-18")
    #[serde(default)]
    pub protocol_version: String,
}

/// MCP tool definition from server
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    #[serde(default)]
    pub description: String,
    /// Input schema (JSON Schema)
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// MCP resource definition
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceDefinition {
    /// Resource URI
    pub uri: String,
    /// Resource name
    pub name: String,
    /// Resource description
    #[serde(default)]
    pub description: Option<String>,
    /// MIME type of the resource
    #[serde(default)]
    pub mime_type: Option<String>,
}

/// MCP server state tracking
#[derive(Clone, Debug)]
pub struct McpServerState {
    /// Server name (unique identifier)
    pub name: String,
    /// Server configuration
    pub config: McpServerConfig,
    /// Connection status
    pub status: McpConnectionStatus,
    /// Server info (available after connection)
    pub server_info: Option<McpServerInfo>,
    /// Available tools
    pub tools: Vec<McpToolDefinition>,
    /// Available resources
    pub resources: Vec<McpResourceDefinition>,
}

impl McpServerState {
    /// Create a new server state with the given name and config
    pub fn new(name: impl Into<String>, config: McpServerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            status: McpConnectionStatus::Connecting,
            server_info: None,
            tools: Vec::new(),
            resources: Vec::new(),
        }
    }

    /// Check if the server is connected
    pub fn is_connected(&self) -> bool {
        self.status == McpConnectionStatus::Connected
    }
}

/// MCP error types
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Connection to server failed
    #[error("Connection failed: {message}")]
    ConnectionFailed {
        /// Error message
        message: String,
    },

    /// Protocol error (invalid messages, etc.)
    #[error("Protocol error: {message}")]
    Protocol {
        /// Error message
        message: String,
    },

    /// JSON-RPC error from server
    #[error("JSON-RPC error {code}: {message}")]
    JsonRpc {
        /// Error code
        code: i32,
        /// Error message
        message: String,
    },

    /// Tool execution error
    #[error("Tool error: {message}")]
    ToolError {
        /// Error message
        message: String,
    },

    /// Version mismatch between client and server
    #[error("Version mismatch: server supports {supported:?}, client requested {requested}")]
    VersionMismatch {
        /// Versions supported by server
        supported: Vec<String>,
        /// Version requested by client
        requested: String,
    },

    /// Server not found
    #[error("Server not found: {name}")]
    ServerNotFound {
        /// Server name
        name: String,
    },

    /// Tool not found
    #[error("Tool not found: {name}")]
    ToolNotFound {
        /// Tool name
        name: String,
    },

    /// Resource not found
    #[error("Resource not found: {uri}")]
    ResourceNotFound {
        /// Resource URI
        uri: String,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type for MCP operations
pub type McpResult<T> = std::result::Result<T, McpError>;

/// MCP tool call result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Result content
    pub content: Vec<McpContent>,
    /// Whether the call resulted in an error
    #[serde(default)]
    pub is_error: bool,
}

/// MCP content types (returned from tool calls and resources)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpContent {
    /// Text content
    Text {
        /// Text value
        text: String,
    },
    /// Image content (base64 encoded)
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
    },
    /// Resource reference
    Resource {
        /// Resource URI
        uri: String,
        /// Resource text content (if available)
        #[serde(default)]
        text: Option<String>,
        /// Resource blob content (if available, base64)
        #[serde(default)]
        blob: Option<String>,
        /// MIME type
        #[serde(default)]
        mime_type: Option<String>,
    },
}

impl McpContent {
    /// Get text content if this is a text content type
    pub fn as_text(&self) -> Option<&str> {
        match self {
            McpContent::Text { text } => Some(text),
            _ => None,
        }
    }
}

impl McpToolResult {
    /// Convert to a string representation
    pub fn to_string_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| c.as_text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mcp_name() {
        assert_eq!(parse_mcp_name("mcp__server_tool"), Some(("server", "tool")));
        assert_eq!(
            parse_mcp_name("mcp__fs_read_file"),
            Some(("fs", "read_file"))
        );
        assert_eq!(parse_mcp_name("Read"), None);
        assert_eq!(parse_mcp_name("mcp_invalid"), None);
    }

    #[test]
    fn test_make_mcp_name() {
        assert_eq!(make_mcp_name("server", "tool"), "mcp__server_tool");
        assert_eq!(make_mcp_name("fs", "read_file"), "mcp__fs_read_file");
    }

    #[test]
    fn test_is_mcp_name() {
        assert!(is_mcp_name("mcp__server_tool"));
        assert!(!is_mcp_name("Read"));
        assert!(!is_mcp_name("mcp_invalid"));
    }

    #[test]
    fn test_reconnect_policy_delay() {
        let policy = ReconnectPolicy::default();
        let d0 = policy.delay_for_attempt(0);
        let d1 = policy.delay_for_attempt(1);
        assert!(d1 > d0);
        assert!(d0.as_millis() >= 1000);
        assert!(d0.as_millis() <= 1300);
    }

    #[test]
    fn test_mcp_server_config_serde() {
        let config = McpServerConfig::Stdio {
            command: "npx".to_string(),
            args: vec!["server".to_string()],
            env: HashMap::new(),
            cwd: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("stdio"));
        assert!(json.contains("npx"));
    }

    #[test]
    fn test_mcp_server_state_new() {
        let state = McpServerState::new(
            "test",
            McpServerConfig::Stdio {
                command: "test".to_string(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
            },
        );

        assert_eq!(state.name, "test");
        assert_eq!(state.status, McpConnectionStatus::Connecting);
        assert!(!state.is_connected());
    }

    #[test]
    fn test_mcp_content_as_text() {
        let content = McpContent::Text {
            text: "hello".to_string(),
        };
        assert_eq!(content.as_text(), Some("hello"));

        let image = McpContent::Image {
            data: "base64".to_string(),
            mime_type: "image/png".to_string(),
        };
        assert_eq!(image.as_text(), None);
    }
}
