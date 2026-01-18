//! Anthropic API client with multi-provider support.

pub mod adapter;
pub mod batch;
pub mod fallback;
pub mod files;
pub mod gateway;
pub mod messages;
pub mod network;
pub mod recovery;
pub mod resilience;
pub mod schema;
mod streaming;

pub use adapter::{
    AnthropicAdapter, BetaConfig, BetaFeature, CloudProvider, DEFAULT_MODEL,
    DEFAULT_REASONING_MODEL, DEFAULT_SMALL_MODEL, FRONTIER_MODEL, ModelConfig, ModelType,
    ProviderAdapter, ProviderConfig,
};
pub use batch::{
    BatchClient, BatchRequest, BatchResult, BatchStatus, CreateBatchRequest, MessageBatch,
};
pub use fallback::{FallbackConfig, FallbackTrigger};
pub use files::{File, FileData, FileDownload, FileListResponse, FilesClient, UploadFileRequest};
pub use gateway::GatewayConfig;
pub use messages::{
    ClearConfig, ClearTrigger, ContextEdit, ContextManagement, CountTokensContextManagement,
    CountTokensRequest, CountTokensResponse, CreateMessageRequest, DEFAULT_MAX_TOKENS, EffortLevel,
    KeepConfig, KeepThinkingConfig, MAX_TOKENS_128K, MIN_MAX_TOKENS, MIN_THINKING_BUDGET,
    OutputConfig, OutputFormat, ThinkingConfig, ThinkingType, TokenValidationError, ToolChoice,
};
pub use network::{ClientCertConfig, NetworkConfig, PoolConfig, ProxyConfig};
pub use recovery::StreamRecoveryState;
pub use resilience::{
    CircuitBreaker, CircuitConfig, CircuitState, ExponentialBackoff, Resilience, ResilienceConfig,
    RetryConfig,
};
pub use schema::{strict_schema, transform_for_strict};
pub use streaming::{RecoverableStream, StreamItem, StreamParser};

#[cfg(feature = "aws")]
pub use adapter::BedrockAdapter;
#[cfg(feature = "azure")]
pub use adapter::FoundryAdapter;
#[cfg(feature = "gcp")]
pub use adapter::VertexAdapter;

use std::sync::Arc;
use std::time::Duration;

use crate::auth::{Auth, Credential, OAuthConfig};
use crate::{Error, Result};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct Client {
    adapter: Arc<dyn ProviderAdapter>,
    http: reqwest::Client,
    fallback_config: Option<FallbackConfig>,
    resilience: Option<Arc<Resilience>>,
}

impl Client {
    pub fn new(adapter: impl ProviderAdapter + 'static) -> Result<Self> {
        let timeout = DEFAULT_TIMEOUT;
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(Error::Network)?;

        Ok(Self {
            adapter: Arc::new(adapter),
            http,
            fallback_config: None,
            resilience: None,
        })
    }

    pub fn with_http(adapter: impl ProviderAdapter + 'static, http: reqwest::Client) -> Self {
        Self {
            adapter: Arc::new(adapter),
            http,
            fallback_config: None,
            resilience: None,
        }
    }

    pub fn with_fallback(mut self, config: FallbackConfig) -> Self {
        self.fallback_config = Some(config);
        self
    }

    pub fn with_resilience(mut self, config: ResilienceConfig) -> Self {
        self.resilience = Some(Arc::new(Resilience::new(config)));
        self
    }

    pub fn resilience(&self) -> Option<&Arc<Resilience>> {
        self.resilience.as_ref()
    }

    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub async fn query(&self, prompt: &str) -> Result<String> {
        self.query_with_model(prompt, ModelType::Primary).await
    }

    pub async fn query_with_model(&self, prompt: &str, model_type: ModelType) -> Result<String> {
        let model = self.adapter.model(model_type).to_string();
        let request = CreateMessageRequest::new(&model, vec![crate::types::Message::user(prompt)])
            .with_max_tokens(self.adapter.config().max_tokens);
        request.validate()?;

        let response = self.adapter.send(&self.http, request).await?;
        Ok(response.text())
    }

    pub async fn send(&self, request: CreateMessageRequest) -> Result<crate::types::ApiResponse> {
        request.validate()?;

        let fallback = match &self.fallback_config {
            Some(f) => f,
            None => return self.adapter.send(&self.http, request).await,
        };

        let mut current_request = request;
        let mut attempt = 0;
        let mut using_fallback = false;

        loop {
            match self.adapter.send(&self.http, current_request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if !fallback.should_fallback(&e) {
                        return Err(e);
                    }

                    attempt += 1;
                    if attempt > fallback.max_retries {
                        return Err(e);
                    }

                    if !using_fallback {
                        tracing::warn!(
                            error = %e,
                            fallback_model = %fallback.fallback_model,
                            attempt,
                            max_retries = fallback.max_retries,
                            "Primary model failed, falling back"
                        );
                        current_request = current_request.with_model(&fallback.fallback_model);
                        using_fallback = true;
                    } else {
                        tracing::warn!(
                            error = %e,
                            attempt,
                            max_retries = fallback.max_retries,
                            "Fallback model failed, retrying"
                        );
                    }
                }
            }
        }
    }

    pub async fn send_no_fallback(
        &self,
        request: CreateMessageRequest,
    ) -> Result<crate::types::ApiResponse> {
        request.validate()?;
        self.adapter.send(&self.http, request).await
    }

    pub fn fallback_config(&self) -> Option<&FallbackConfig> {
        self.fallback_config.as_ref()
    }

    pub async fn stream(
        &self,
        prompt: &str,
    ) -> Result<impl futures::Stream<Item = Result<String>> + Send + 'static + use<>> {
        let model = self.adapter.model(ModelType::Primary).to_string();
        let request = CreateMessageRequest::new(&model, vec![crate::types::Message::user(prompt)])
            .with_max_tokens(self.adapter.config().max_tokens);
        request.validate()?;

        let response = self.adapter.send_stream(&self.http, request).await?;
        let stream = StreamParser::new(response.bytes_stream());

        Ok(futures::StreamExt::filter_map(stream, |item| async move {
            match item {
                Ok(StreamItem::Text(text)) => Some(Ok(text)),
                Ok(StreamItem::Thinking(text)) => Some(Ok(text)),
                Ok(
                    StreamItem::Event(_) | StreamItem::Citation(_) | StreamItem::ToolUseComplete(_),
                ) => None,
                Err(e) => Some(Err(e)),
            }
        }))
    }

    pub async fn stream_request(
        &self,
        request: CreateMessageRequest,
    ) -> Result<impl futures::Stream<Item = Result<StreamItem>> + Send + 'static + use<>> {
        request.validate()?;
        let response = self.adapter.send_stream(&self.http, request).await?;
        Ok(StreamParser::new(response.bytes_stream()))
    }

    pub async fn stream_recoverable(
        &self,
        request: CreateMessageRequest,
    ) -> Result<
        RecoverableStream<
            impl futures::Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>>
            + Send
            + 'static
            + use<>,
        >,
    > {
        request.validate()?;
        let response = self.adapter.send_stream(&self.http, request).await?;
        Ok(RecoverableStream::new(response.bytes_stream()))
    }

    pub async fn stream_with_recovery(
        &self,
        request: CreateMessageRequest,
        recovery_state: Option<StreamRecoveryState>,
    ) -> Result<
        RecoverableStream<
            impl futures::Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>>
            + Send
            + 'static
            + use<>,
        >,
    > {
        let request = match recovery_state {
            Some(state) if state.is_recoverable() => {
                let mut req = request;
                req.messages = state.build_continuation_messages(&req.messages);
                req
            }
            _ => request,
        };
        self.stream_recoverable(request).await
    }

    pub fn batch(&self) -> BatchClient<'_> {
        BatchClient::new(self)
    }

    pub fn files(&self) -> FilesClient<'_> {
        FilesClient::new(self)
    }

    pub fn adapter(&self) -> &dyn ProviderAdapter {
        self.adapter.as_ref()
    }

    pub fn config(&self) -> &ProviderConfig {
        self.adapter.config()
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub async fn refresh_credentials(&self) -> Result<()> {
        self.adapter.refresh_credentials().await
    }

    pub async fn send_with_auth_retry(
        &self,
        request: CreateMessageRequest,
    ) -> Result<crate::types::ApiResponse> {
        self.adapter.ensure_fresh_credentials().await?;

        match self.send(request.clone()).await {
            Ok(resp) => Ok(resp),
            Err(e) if e.is_unauthorized() && self.adapter.supports_credential_refresh() => {
                tracing::debug!("Received 401, refreshing credentials");
                self.refresh_credentials().await?;
                self.send(request).await
            }
            Err(e) => Err(e),
        }
    }

    pub async fn send_stream_with_auth_retry(
        &self,
        request: CreateMessageRequest,
    ) -> Result<reqwest::Response> {
        request.validate()?;
        self.adapter.ensure_fresh_credentials().await?;

        match self.adapter.send_stream(&self.http, request.clone()).await {
            Ok(resp) => Ok(resp),
            Err(e) if e.is_unauthorized() && self.adapter.supports_credential_refresh() => {
                tracing::debug!("Received 401, refreshing credentials");
                self.refresh_credentials().await?;
                self.adapter.send_stream(&self.http, request).await
            }
            Err(e) => Err(e),
        }
    }

    pub async fn count_tokens(
        &self,
        request: messages::CountTokensRequest,
    ) -> Result<messages::CountTokensResponse> {
        self.adapter.ensure_fresh_credentials().await?;

        match self.adapter.count_tokens(&self.http, request.clone()).await {
            Ok(resp) => Ok(resp),
            Err(e) if e.is_unauthorized() && self.adapter.supports_credential_refresh() => {
                self.refresh_credentials().await?;
                self.adapter.count_tokens(&self.http, request).await
            }
            Err(e) => Err(e),
        }
    }

    pub async fn count_tokens_for_request(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<messages::CountTokensResponse> {
        let count_request = messages::CountTokensRequest::from_message_request(request);
        self.count_tokens(count_request).await
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("provider", &self.adapter.name())
            .finish()
    }
}

#[derive(Default)]
pub struct ClientBuilder {
    provider: Option<CloudProvider>,
    credential: Option<Credential>,
    credential_provider: Option<Arc<dyn crate::auth::CredentialProvider>>,
    oauth_config: Option<OAuthConfig>,
    config: Option<ProviderConfig>,
    models: Option<ModelConfig>,
    network: Option<NetworkConfig>,
    gateway: Option<GatewayConfig>,
    timeout: Option<Duration>,
    fallback_config: Option<FallbackConfig>,
    resilience_config: Option<ResilienceConfig>,

    #[cfg(feature = "aws")]
    aws_region: Option<String>,
    #[cfg(feature = "gcp")]
    gcp_project: Option<String>,
    #[cfg(feature = "gcp")]
    gcp_region: Option<String>,
    #[cfg(feature = "azure")]
    azure_resource: Option<String>,
}

impl ClientBuilder {
    /// Configure authentication for the client.
    ///
    /// Accepts `Auth` enum or any type that converts to it (e.g., API key string).
    /// For `Auth::ClaudeCli`, the credential provider is preserved for automatic token refresh.
    pub async fn auth(mut self, auth: impl Into<Auth>) -> Result<Self> {
        let auth = auth.into();

        #[allow(unreachable_patterns)]
        match &auth {
            #[cfg(feature = "aws")]
            Auth::Bedrock { region } => {
                self.provider = Some(CloudProvider::Bedrock);
                self.aws_region = Some(region.clone());
            }
            #[cfg(feature = "gcp")]
            Auth::Vertex { project, region } => {
                self.provider = Some(CloudProvider::Vertex);
                self.gcp_project = Some(project.clone());
                self.gcp_region = Some(region.clone());
            }
            #[cfg(feature = "azure")]
            Auth::Foundry { resource } => {
                self.provider = Some(CloudProvider::Foundry);
                self.azure_resource = Some(resource.clone());
            }
            _ => {
                self.provider = Some(CloudProvider::Anthropic);
            }
        }

        let (credential, provider) = auth.resolve_with_provider().await?;
        if !credential.is_default() {
            self.credential = Some(credential);
        }
        self.credential_provider = provider;

        Ok(self)
    }

    pub fn anthropic(mut self) -> Self {
        self.provider = Some(CloudProvider::Anthropic);
        self
    }

    #[cfg(feature = "aws")]
    pub(crate) fn with_aws_region(mut self, region: String) -> Self {
        self.provider = Some(CloudProvider::Bedrock);
        self.aws_region = Some(region);
        self
    }

    #[cfg(feature = "gcp")]
    pub(crate) fn with_gcp(mut self, project: String, region: String) -> Self {
        self.provider = Some(CloudProvider::Vertex);
        self.gcp_project = Some(project);
        self.gcp_region = Some(region);
        self
    }

    #[cfg(feature = "azure")]
    pub(crate) fn with_azure_resource(mut self, resource: String) -> Self {
        self.provider = Some(CloudProvider::Foundry);
        self.azure_resource = Some(resource);
        self
    }

    pub fn oauth_config(mut self, config: OAuthConfig) -> Self {
        self.oauth_config = Some(config);
        self
    }

    pub fn models(mut self, models: ModelConfig) -> Self {
        self.models = Some(models);
        self
    }

    pub fn config(mut self, config: ProviderConfig) -> Self {
        self.config = Some(config);
        self
    }

    pub fn network(mut self, network: NetworkConfig) -> Self {
        self.network = Some(network);
        self
    }

    pub fn gateway(mut self, gateway: GatewayConfig) -> Self {
        self.gateway = Some(gateway);
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn fallback(mut self, config: FallbackConfig) -> Self {
        self.fallback_config = Some(config);
        self
    }

    pub fn fallback_model(mut self, model: impl Into<String>) -> Self {
        self.fallback_config = Some(FallbackConfig::new(model));
        self
    }

    pub fn resilience(mut self, config: ResilienceConfig) -> Self {
        self.resilience_config = Some(config);
        self
    }

    pub fn with_default_resilience(mut self) -> Self {
        self.resilience_config = Some(ResilienceConfig::default());
        self
    }

    pub async fn build(self) -> Result<Client> {
        let provider = self.provider.unwrap_or_else(CloudProvider::from_env);

        let models = self.models.unwrap_or_else(|| provider.default_models());

        let config = self.config.unwrap_or_else(|| ProviderConfig::new(models));

        let adapter: Box<dyn ProviderAdapter> = match provider {
            CloudProvider::Anthropic => {
                let adapter = if let Some(ref cred) = self.credential {
                    let mut a = if let Some(cred_provider) = self.credential_provider {
                        AnthropicAdapter::from_credential_provider(
                            config,
                            cred,
                            self.oauth_config,
                            cred_provider,
                        )
                    } else {
                        AnthropicAdapter::from_credential(config, cred, self.oauth_config)
                    };
                    if let Some(ref gw) = self.gateway
                        && let Some(ref url) = gw.base_url
                    {
                        a = a.with_base_url(url);
                    }
                    a
                } else {
                    let mut a = AnthropicAdapter::new(config);
                    if let Some(ref gw) = self.gateway {
                        if let Some(ref url) = gw.base_url {
                            a = a.with_base_url(url);
                        }
                        if let Some(ref token) = gw.auth_token {
                            a = a.with_api_key(token);
                        }
                    }
                    a
                };
                Box::new(adapter)
            }
            #[cfg(feature = "aws")]
            CloudProvider::Bedrock => {
                let mut adapter = adapter::BedrockAdapter::from_env(config).await?;
                if let Some(region) = self.aws_region {
                    adapter = adapter.with_region(region);
                }
                Box::new(adapter)
            }
            #[cfg(feature = "gcp")]
            CloudProvider::Vertex => {
                let mut adapter = adapter::VertexAdapter::from_env(config).await?;
                if let Some(project) = self.gcp_project {
                    adapter = adapter.with_project(project);
                }
                if let Some(region) = self.gcp_region {
                    adapter = adapter.with_region(region);
                }
                Box::new(adapter)
            }
            #[cfg(feature = "azure")]
            CloudProvider::Foundry => {
                let mut adapter = adapter::FoundryAdapter::from_env(config).await?;
                if let Some(resource) = self.azure_resource {
                    adapter = adapter.with_resource(resource);
                }
                Box::new(adapter)
            }
        };

        let mut http_builder =
            reqwest::Client::builder().timeout(self.timeout.unwrap_or(DEFAULT_TIMEOUT));

        if let Some(ref network) = self.network {
            http_builder = network
                .apply_to_builder(http_builder)
                .map_err(|e| Error::Config(e.to_string()))?;
        }

        let http = http_builder.build().map_err(Error::Network)?;

        let resilience = self.resilience_config.map(|c| Arc::new(Resilience::new(c)));

        Ok(Client {
            adapter: Arc::from(adapter),
            http,
            fallback_config: self.fallback_config,
            resilience,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder() {
        let _builder = Client::builder().anthropic();
    }

    #[test]
    fn test_cloud_provider_from_env() {
        let provider = CloudProvider::from_env();
        assert_eq!(provider, CloudProvider::Anthropic);
    }

    #[tokio::test]
    async fn test_builder_with_auth_credential() {
        let _builder = Client::builder()
            .anthropic()
            .auth(Credential::api_key("test-key"))
            .await
            .unwrap();
    }
}
