//! Client error types.

use thiserror::Error;

/// Errors specific to the API client
#[derive(Debug, Error)]
pub enum ClientError {
    /// HTTP request failed
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// API returned an error response
    #[error("API error ({status}): {message}")]
    ApiError {
        /// HTTP status code
        status: u16,
        /// Error message from API
        message: String,
        /// Error type from API
        error_type: Option<String>,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimited {
        /// Retry after duration
        retry_after: Option<std::time::Duration>,
    },

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthError(String),

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Stream parsing error
    #[error("Stream error: {0}")]
    StreamError(String),
}

impl ClientError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ClientError::RateLimited { .. }
                | ClientError::Http(_)
                | ClientError::ApiError { status: 500..=599, .. }
        )
    }

    /// Get retry delay if applicable
    pub fn retry_after(&self) -> Option<std::time::Duration> {
        match self {
            ClientError::RateLimited { retry_after } => *retry_after,
            _ => None,
        }
    }
}
