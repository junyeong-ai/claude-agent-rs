//! Authentication strategy trait.

use std::fmt::Debug;

use crate::client::messages::{CreateMessageRequest, RequestMetadata};
use crate::types::SystemPrompt;

/// Authentication strategy interface.
/// Encapsulates all authentication-specific request configuration.
pub trait AuthStrategy: Send + Sync + Debug {
    /// Returns the authentication header (name, value).
    fn auth_header(&self) -> (&'static str, String);

    /// Returns additional headers required by this strategy.
    fn extra_headers(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Returns URL query parameters (e.g., "api-version=2024-06-01").
    fn url_query_string(&self) -> Option<String> {
        None
    }

    /// Prepares the system prompt for the request.
    fn prepare_system_prompt(&self, existing: Option<SystemPrompt>) -> Option<SystemPrompt> {
        existing
    }

    /// Generates request metadata if required.
    fn prepare_metadata(&self) -> Option<RequestMetadata> {
        None
    }

    /// Prepares the full request with all strategy-specific modifications.
    fn prepare_request(&self, mut request: CreateMessageRequest) -> CreateMessageRequest {
        if request.metadata.is_none() {
            request.metadata = self.prepare_metadata();
        }
        request.system = self.prepare_system_prompt(request.system);
        request
    }

    /// Returns the strategy name for logging/debugging.
    fn name(&self) -> &'static str;
}
