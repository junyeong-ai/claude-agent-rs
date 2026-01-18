//! Tool definition types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
}

impl ToolDefinition {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            strict: None,
            defer_loading: None,
        }
    }

    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = Some(strict);
        self
    }

    pub fn with_defer_loading(mut self, defer: bool) -> Self {
        self.defer_loading = Some(defer);
        self
    }

    pub fn deferred(mut self) -> Self {
        self.defer_loading = Some(true);
        self
    }

    pub fn is_deferred(&self) -> bool {
        self.defer_loading.unwrap_or(false)
    }

    pub fn estimated_tokens(&self) -> usize {
        let name_tokens = self.name.len() / 4;
        let desc_tokens = self.description.len() / 4;
        let schema_tokens = self.input_schema.to_string().len() / 4;
        name_tokens + desc_tokens + schema_tokens + 20
    }
}
