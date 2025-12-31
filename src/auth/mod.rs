//! Authentication module for Claude API.
//!
//! Provides multiple authentication strategies:
//! - **API Key**: Simple authentication via `x-api-key` header
//! - **OAuth**: Claude Code CLI token authentication
//! - **Bedrock**: AWS Bedrock authentication
//! - **Vertex**: Google Vertex AI authentication
//! - **Foundry**: Microsoft Azure AI Foundry authentication

mod config;
mod credential;
mod provider;
mod providers;
mod storage;
mod strategy;

pub use config::{OAuthConfig, OAuthConfigBuilder};
pub use credential::{Credential, OAuthCredential};
pub use provider::CredentialProvider;
pub use providers::{ChainProvider, ClaudeCliProvider, EnvironmentProvider, ExplicitProvider};
pub use storage::CliCredentials;
pub use strategy::{
    ApiKeyStrategy, AuthStrategy, BedrockStrategy, FoundryStrategy, OAuthStrategy, VertexStrategy,
};
