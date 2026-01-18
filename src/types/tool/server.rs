//! Anthropic built-in server-side tool configurations.

use serde::{Deserialize, Serialize};

use crate::types::citations::CitationsConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_location: Option<UserLocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLocation {
    #[serde(rename = "type")]
    pub location_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self {
            tool_type: "web_search_20250305".to_string(),
            name: "web_search".to_string(),
            max_uses: None,
            allowed_domains: None,
            blocked_domains: None,
            user_location: None,
        }
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }

    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = Some(domains);
        self
    }

    pub fn with_blocked_domains(mut self, domains: Vec<String>) -> Self {
        self.blocked_domains = Some(domains);
        self
    }

    pub fn with_user_location(mut self, location: UserLocation) -> Self {
        self.user_location = Some(location);
        self
    }
}

impl UserLocation {
    pub fn approximate(country: impl Into<String>) -> Self {
        Self {
            location_type: "approximate".to_string(),
            city: None,
            region: None,
            country: Some(country.into()),
            timezone: None,
        }
    }

    pub fn with_city(mut self, city: impl Into<String>) -> Self {
        self.city = Some(city.into());
        self
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    pub fn with_timezone(mut self, timezone: impl Into<String>) -> Self {
        self.timezone = Some(timezone.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_domains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_content_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<CitationsConfig>,
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self {
            tool_type: "web_fetch_20250910".to_string(),
            name: "web_fetch".to_string(),
            max_uses: None,
            allowed_domains: None,
            blocked_domains: None,
            max_content_tokens: None,
            citations: None,
        }
    }
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }

    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = Some(domains);
        self
    }

    pub fn with_blocked_domains(mut self, domains: Vec<String>) -> Self {
        self.blocked_domains = Some(domains);
        self
    }

    pub fn with_max_content_tokens(mut self, tokens: u32) -> Self {
        self.max_content_tokens = Some(tokens);
        self
    }

    pub fn with_citations(mut self, enabled: bool) -> Self {
        self.citations = Some(if enabled {
            CitationsConfig::enabled()
        } else {
            CitationsConfig::disabled()
        });
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolSearchTool {
    #[serde(rename = "tool_search_tool_regex_20251119")]
    Regex { name: String },
    #[serde(rename = "tool_search_tool_bm25_20251119")]
    Bm25 { name: String },
}

impl ToolSearchTool {
    pub fn regex() -> Self {
        Self::Regex {
            name: "tool_search_tool_regex".to_string(),
        }
    }

    pub fn bm25() -> Self {
        Self::Bm25 {
            name: "tool_search_tool_bm25".to_string(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Regex { name } | Self::Bm25 { name } => name,
        }
    }

    pub fn is_regex(&self) -> bool {
        matches!(self, Self::Regex { .. })
    }

    pub fn is_bm25(&self) -> bool {
        matches!(self, Self::Bm25 { .. })
    }
}

impl Default for ToolSearchTool {
    fn default() -> Self {
        Self::regex()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ServerTool {
    WebSearch(WebSearchTool),
    WebFetch(WebFetchTool),
    ToolSearch(ToolSearchTool),
}
