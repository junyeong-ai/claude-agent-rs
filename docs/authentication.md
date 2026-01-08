# Authentication

claude-agent-rs supports multiple authentication methods with automatic credential resolution.

## Authentication Flow

```
┌────────────────────────────────────────────────────────────────┐
│                    Credential Resolution                        │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  from_claude_code()                                              │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────┐                                        │
│  │ ClaudeCliProvider   │ ~/.claude/.credentials.json            │
│  │  ├── Load OAuth     │                                        │
│  │  ├── Check expiry   │                                        │
│  │  └── Auto refresh   │ ← `claude auth refresh`                │
│  └──────────┬──────────┘                                        │
│             │                                                   │
│             ▼                                                   │
│     ┌───────────────┐                                           │
│     │ Credential    │                                           │
│     └───────┬───────┘                                           │
│             │                                                   │
│       ┌─────┴─────┐                                             │
│       ▼           ▼                                             │
│   ┌───────┐   ┌───────┐                                         │
│   │ OAuth │   │ApiKey │                                         │
│   └───┬───┘   └───┬───┘                                         │
│       │           │                                             │
│       ▼           ▼                                             │
│  Authorization  x-api-key                                       │
│  Bearer token   header                                          │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

## Claude Code CLI (Recommended)

Uses existing Claude Code CLI OAuth tokens with automatic refresh.

```rust
// Agent (with working directory)
let agent = Agent::builder()
    .from_claude_code(".").await?  // Auth + working_dir
    .build()
    .await?;

// Or with explicit auth
use claude_agent::Auth;
let agent = Agent::builder()
    .auth(Auth::claude_cli()).await?
    .build()
    .await?;
```

**Prerequisite**: Run `claude login` first.

**Automatically included**:
- OAuth Bearer token authentication
- Prompt Caching (`cache_control: ephemeral`)
- Required beta headers (`claude-code-20250219`, `oauth-2025-04-20`)
- Automatic token refresh when expired

**Credential location**: `~/.claude/.credentials.json` (also macOS Keychain)

### Separating Auth from Resources

`from_claude_code()` combines OAuth auth + working_dir + resource loading. You can separate these to use different auth methods while still loading `.claude/` resources:

```rust
// Use API Key with .claude/ resources
Agent::builder()
    .auth(Auth::api_key("sk-...")).await?
    .working_dir("./my-project")
    .with_project_resources()  // Loads .claude/ (skills, rules, CLAUDE.md)
    .build()
    .await?

// Use Bedrock with .claude/ resources
Agent::builder()
    .auth(Auth::bedrock("us-east-1")).await?
    .working_dir("./my-project")
    .with_project_resources()
    .with_user_resources()     // Also load ~/.claude/
    .build()
    .await?
```

This allows using Claude Code's project configuration with any authentication method.

### Token File Structure

```json
{
  "claudeAiOauth": {
    "accessToken": "sk-ant-oat01-...",
    "refreshToken": "sk-ant-ort01-...",
    "expiresAt": 1234567890,
    "scopes": ["user:inference"],
    "subscriptionType": "pro"
  }
}
```

### OAuth HTTP Headers

When using OAuth authentication, these headers are sent:

| Header | Value | Description |
|--------|-------|-------------|
| `Authorization` | `Bearer {token}` | OAuth access token |
| `anthropic-version` | `2023-06-01` | API version |
| `content-type` | `application/json` | Request format |
| `user-agent` | `claude-cli/2.0.76 (external, cli)` | Client identifier |
| `x-app` | `cli` | Application identifier |
| `anthropic-beta` | `oauth-2025-04-20,claude-code-20250219` | Beta features |
| `anthropic-dangerous-direct-browser-access` | `true` | OAuth requirement |

**URL Parameter**: `?beta=true`

### OAuthConfig Customization

```rust
use claude_agent::auth::OAuthConfig;

let oauth_config = OAuthConfig::builder()
    .user_agent("my-app/1.0")
    .app_identifier("my-app")
    .header("x-custom", "value")
    .url_param("custom_param", "value")
    .build();
```

**Environment variables**:
- `CLAUDE_AGENT_USER_AGENT`: Override user-agent
- `CLAUDE_AGENT_APP_IDENTIFIER`: Override x-app

### Token Expiry and Refresh

- Token expiry checked via `expires_at` timestamp
- Auto-refresh triggered 5 minutes before expiry (`needs_refresh()`)
- Refresh runs `claude auth refresh` CLI command

## API Key

Direct API key authentication.

```rust
let client = Client::builder()
    .api_key("sk-ant-api03-...")
    .build()?;
```

## Environment Variable

```rust
// Uses ANTHROPIC_API_KEY
let client = Client::from_env()?;
```

## Cloud Providers

### AWS Bedrock

```rust
let client = Client::builder()
    .bedrock("us-east-1")
    .build()?;
```

**Authentication**: Uses AWS credential chain (environment, config, IAM role).

**Environment variables**:
- `AWS_REGION` or `AWS_DEFAULT_REGION`
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`
- `CLAUDE_CODE_USE_BEDROCK=1` (optional flag)

### Google Vertex AI

```rust
let client = Client::builder()
    .vertex("my-gcp-project", "us-central1")
    .build()?;
```

**Authentication**: Uses Google Application Default Credentials.

**Environment variables**:
- `GOOGLE_APPLICATION_CREDENTIALS` (service account key)
- `GOOGLE_CLOUD_PROJECT`
- `CLAUDE_CODE_USE_VERTEX=1` (optional flag)

### Azure AI Foundry

```rust
let client = Client::builder()
    .foundry("my-resource", "claude-sonnet")
    .build()?;
```

**Authentication**: Uses Azure Identity chain.

**Environment variables**:
- `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID`
- `CLAUDE_CODE_USE_FOUNDRY=1` (optional flag)

## Credential Provider Chain

Custom credential resolution chain.

```rust
use claude_agent::auth::{ChainProvider, ClaudeCliProvider, EnvironmentProvider};

let chain = ChainProvider::new()
    .with(ClaudeCliProvider::new())
    .with(EnvironmentProvider::new());

let client = Client::builder()
    .credential_provider(chain)
    .build()?;
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | API key for direct authentication |
| `ANTHROPIC_MODEL` | Default model (default: `claude-sonnet-4-5`) |
| `ANTHROPIC_BASE_URL` | Custom API endpoint |
| `CLAUDE_CODE_USE_BEDROCK` | Enable AWS Bedrock |
| `CLAUDE_CODE_USE_VERTEX` | Enable Google Vertex AI |
| `CLAUDE_CODE_USE_FOUNDRY` | Enable Azure Foundry |

## Token Refresh

OAuth tokens from Claude Code CLI are automatically refreshed:

1. On initialization, check token expiry
2. If expired, call `claude auth refresh`
3. Reload credentials from disk
4. Return fresh token

```rust
// ClaudeCliProvider handles this automatically
async fn refresh(&self) -> Result<Credential> {
    Command::new("claude")
        .args(["auth", "refresh"])
        .output()
        .await?;
    // Reload from disk
}
```

## Security Considerations

- OAuth tokens are stored in `~/.claude/credentials.json`
- API keys should use environment variables, not hardcoded
- Cloud providers use their native credential chains
- Token refresh happens automatically before expiry
