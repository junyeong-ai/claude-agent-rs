# Cloud Providers

claude-agent-rs supports multiple cloud platforms through the ProviderAdapter system.

> **Related**: [Authentication Guide](authentication.md) for credential resolution and OAuth setup

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Provider Adapters                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │  Anthropic  │  │   Bedrock   │  │   Vertex    │          │
│  │   (Direct)  │  │    (AWS)    │  │  (Google)   │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         │                │                │                  │
│         └────────────────┼────────────────┘                  │
│                          │                                   │
│                          ▼                                   │
│                 ┌─────────────────┐                          │
│                 │ ProviderAdapter │                          │
│                 │     trait       │                          │
│                 └────────┬────────┘                          │
│                          │                                   │
│         ┌────────────────┼────────────────┐                  │
│         ▼                ▼                ▼                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ build_url() │  │ transform   │  │ apply_auth  │          │
│  │             │  │  _request() │  │  _headers() │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
│                                                              │
│  ┌─────────────┐                                             │
│  │   Foundry   │                                             │
│  │   (Azure)   │                                             │
│  └─────────────┘                                             │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Anthropic (Direct API)

Direct connection to Anthropic's API.

```rust
// Using API key
let client = Client::builder()
    .auth("sk-ant-api03-...")
    .await?
    .build()
    .await?;

// Using environment variable
let client = Client::builder()
    .auth(Auth::FromEnv)
    .await?
    .build()
    .await?;

// Using Claude Code CLI credentials
let client = Client::builder()
    .auth(Auth::ClaudeCli)
    .await?
    .build()
    .await?;
```

### Configuration

| Environment Variable | Description |
|---------------------|-------------|
| `ANTHROPIC_API_KEY` | API key |
| `ANTHROPIC_MODEL` | Default model |
| `ANTHROPIC_BASE_URL` | Custom endpoint |

### Request Format

```http
POST https://api.anthropic.com/v1/messages
Authorization: Bearer {token}  (OAuth)
x-api-key: {key}               (API key)
anthropic-version: 2023-06-01
anthropic-beta: prompt-caching-2024-07-31
```

## AWS Bedrock

Claude models via Amazon Bedrock.

```rust
// Using Auth enum (recommended)
let client = Client::builder()
    .auth(Auth::Bedrock { region: "us-east-1".into() })
    .await?
    .build()
    .await?;
```

### Configuration

| Environment Variable | Description |
|---------------------|-------------|
| `AWS_REGION` | AWS region |
| `AWS_ACCESS_KEY_ID` | Access key |
| `AWS_SECRET_ACCESS_KEY` | Secret key |
| `AWS_SESSION_TOKEN` | Session token (optional) |
| `CLAUDE_CODE_USE_BEDROCK` | Enable flag |

### Authentication

Uses AWS credential chain:
1. Environment variables
2. AWS config file (`~/.aws/credentials`)
3. IAM role (EC2, ECS, Lambda)
4. Web identity token

### Request Format

```http
POST https://bedrock-runtime.{region}.amazonaws.com/model/{model}/invoke
Authorization: AWS4-HMAC-SHA256 ...
```

### Model Mapping

| Standard | Bedrock |
|----------|---------|
| claude-sonnet-4-5 | anthropic.claude-sonnet-4-5-v1 |
| claude-haiku-4-5 | anthropic.claude-haiku-4-5-v1 |
| claude-opus-4-5 | anthropic.claude-opus-4-5-v1 |

## Google Vertex AI

Claude models via Google Cloud Vertex AI.

```rust
let client = Client::builder()
    .auth(Auth::Vertex {
        project: "my-gcp-project".into(),
        region: "us-central1".into(),
    })
    .await?
    .build()
    .await?;
```

### Configuration

| Environment Variable | Description |
|---------------------|-------------|
| `GOOGLE_CLOUD_PROJECT` | GCP project ID |
| `GOOGLE_APPLICATION_CREDENTIALS` | Service account key path |
| `CLOUD_ML_REGION` | Vertex AI region |
| `CLAUDE_CODE_USE_VERTEX` | Enable flag |

### Authentication

Uses Application Default Credentials:
1. `GOOGLE_APPLICATION_CREDENTIALS` environment variable
2. gcloud CLI credentials
3. Compute Engine metadata service
4. GKE workload identity

### Request Format

```http
POST https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/publishers/anthropic/models/{model}:streamRawPredict
Authorization: Bearer {oauth_token}
```

### Model Mapping

| Standard | Vertex |
|----------|--------|
| claude-sonnet-4-5 | claude-sonnet-4-5@20250929 |
| claude-haiku-4-5 | claude-haiku-4-5@20251001 |
| claude-opus-4-5 | claude-opus-4-5@20251101 |

## Azure AI Foundry

Claude models via Azure AI Foundry.

```rust
let client = Client::builder()
    .auth(Auth::Foundry { resource: "my-resource".into() })
    .await?
    .build()
    .await?;
```

### Configuration

| Environment Variable | Description |
|---------------------|-------------|
| `AZURE_CLIENT_ID` | Service principal client ID |
| `AZURE_CLIENT_SECRET` | Service principal secret |
| `AZURE_TENANT_ID` | Azure AD tenant ID |
| `AZURE_SUBSCRIPTION_ID` | Azure subscription |
| `CLAUDE_CODE_USE_FOUNDRY` | Enable flag |

### Authentication

Uses Azure Identity chain:
1. Environment credentials
2. Managed identity
3. Azure CLI credentials
4. Workload identity

### Request Format

```http
POST https://{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions
Authorization: Bearer {azure_token}
api-version: 2024-02-01
```

## Provider Adapter Trait

```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync + Debug {
    fn config(&self) -> &ProviderConfig;
    fn name(&self) -> &'static str;
    fn model(&self, model_type: ModelType) -> &str;
    fn build_url(&self, model: &str, stream: bool) -> String;
    fn prepare_request(&self, request: CreateMessageRequest) -> CreateMessageRequest;
    fn transform_request(&self, request: CreateMessageRequest) -> serde_json::Value;
    fn transform_response(&self, response: serde_json::Value) -> Result<ApiResponse>;
    fn apply_auth_headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder;
    async fn send(&self, http: &reqwest::Client, request: CreateMessageRequest) -> Result<ApiResponse>;
    async fn send_stream(&self, http: &reqwest::Client, request: CreateMessageRequest) -> Result<reqwest::Response>;
    async fn refresh_credentials(&self) -> Result<()>;
}
```

## Model Types

```rust
pub enum ModelType {
    Default,  // Main model for conversations
    Haiku,    // Fast model for simple tasks
    Opus,     // Powerful model for complex tasks
}
```

## Auto-Detection

Environment-based provider selection:

```rust
let client = Client::from_env()?;

// Checks in order:
// 1. CLAUDE_CODE_USE_BEDROCK → Bedrock
// 2. CLAUDE_CODE_USE_VERTEX → Vertex
// 3. CLAUDE_CODE_USE_FOUNDRY → Foundry
// 4. ANTHROPIC_API_KEY → Direct Anthropic
```

## Token Caching

Providers cache authentication tokens:

```rust
// Automatic refresh before expiry
// Bedrock: STS tokens (1 hour)
// Vertex: OAuth tokens (1 hour)
// Foundry: Azure AD tokens (1 hour)
```

## Best Practices

1. **Use environment variables**: Don't hardcode credentials
2. **Prefer managed identity**: Use IAM roles, workload identity
3. **Set region close to users**: Reduce latency
4. **Monitor quotas**: Cloud providers have rate limits
5. **Enable logging**: Track API usage and errors
