//! Credential types.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// OAuth credential from Claude Code CLI.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCredential {
    /// Access token.
    pub access_token: String,
    /// Refresh token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Expiration timestamp (Unix seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    /// Token scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Subscription type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
}

impl OAuthCredential {
    /// Get expiration as DateTime.
    pub fn expires_at_datetime(&self) -> Option<DateTime<Utc>> {
        self.expires_at
            .map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now))
    }

    /// Check if token is expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at_datetime()
            .map(|exp| Utc::now() >= exp)
            .unwrap_or(false)
    }

    /// Check if token needs refresh (within 5 minutes of expiry).
    pub fn needs_refresh(&self) -> bool {
        self.expires_at_datetime()
            .map(|exp| Utc::now() >= exp - Duration::minutes(5))
            .unwrap_or(false)
    }
}

/// Authentication credential.
#[derive(Clone, Debug)]
pub enum Credential {
    /// API Key authentication.
    ApiKey(String),
    /// OAuth token authentication.
    OAuth(OAuthCredential),
}

impl Credential {
    /// Create API Key credential.
    pub fn api_key(key: impl Into<String>) -> Self {
        Self::ApiKey(key.into())
    }

    /// Create OAuth credential.
    pub fn oauth(token: impl Into<String>) -> Self {
        Self::OAuth(OAuthCredential {
            access_token: token.into(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            subscription_type: None,
        })
    }

    /// Check if credential is expired.
    pub fn is_expired(&self) -> bool {
        match self {
            Credential::ApiKey(_) => false,
            Credential::OAuth(oauth) => oauth.is_expired(),
        }
    }

    /// Check if credential needs refresh.
    pub fn needs_refresh(&self) -> bool {
        match self {
            Credential::ApiKey(_) => false,
            Credential::OAuth(oauth) => oauth.needs_refresh(),
        }
    }

    /// Get credential type name.
    pub fn credential_type(&self) -> &'static str {
        match self {
            Credential::ApiKey(_) => "api_key",
            Credential::OAuth(_) => "oauth",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_credential() {
        let cred = Credential::api_key("sk-ant-api-test");
        assert!(!cred.is_expired());
        assert!(!cred.needs_refresh());
        assert_eq!(cred.credential_type(), "api_key");
    }

    #[test]
    fn test_oauth_credential() {
        let cred = Credential::oauth("sk-ant-oat01-test");
        assert_eq!(cred.credential_type(), "oauth");
    }

    #[test]
    fn test_oauth_expiry() {
        let expired = OAuthCredential {
            access_token: "test".into(),
            refresh_token: None,
            expires_at: Some(0),
            scopes: vec![],
            subscription_type: None,
        };
        assert!(expired.is_expired());

        let future = OAuthCredential {
            access_token: "test".into(),
            refresh_token: None,
            expires_at: Some(Utc::now().timestamp() + 3600),
            scopes: vec![],
            subscription_type: None,
        };
        assert!(!future.is_expired());
    }
}
