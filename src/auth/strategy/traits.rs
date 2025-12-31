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
    fn extra_headers(&self) -> Vec<(String, String)>;

    /// Returns URL query parameters (e.g., "beta=true").
    fn url_query_string(&self) -> Option<String>;

    /// Prepares the system prompt for the request.
    fn prepare_system_prompt(&self, existing: Option<SystemPrompt>) -> Option<SystemPrompt>;

    /// Generates request metadata if required.
    fn prepare_metadata(&self) -> Option<RequestMetadata>;

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
