//! Credential types.

use std::fmt;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// OAuth credential from Claude Code CLI.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCredential {
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
}

impl fmt::Debug for OAuthCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthCredential")
            .field("access_token", &"[redacted]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[redacted]"),
            )
            .field("expires_at", &self.expires_at)
            .field("scopes", &self.scopes)
            .field("subscription_type", &self.subscription_type)
            .finish()
    }
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
#[derive(Clone)]
pub enum Credential {
    ApiKey(String),
    OAuth(OAuthCredential),
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey(_) => f.debug_tuple("ApiKey").field(&"[redacted]").finish(),
            Self::OAuth(oauth) => f.debug_tuple("OAuth").field(oauth).finish(),
        }
    }
}

impl Default for Credential {
    /// Default credential is an empty API key (placeholder for cloud providers).
    fn default() -> Self {
        Self::ApiKey(String::new())
    }
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

    /// Check if this is a default (empty) credential.
    /// Used for cloud providers that handle auth differently.
    pub fn is_default(&self) -> bool {
        match self {
            Self::ApiKey(key) => key.is_empty(),
            Self::OAuth(oauth) => oauth.access_token.is_empty(),
        }
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

    /// Check if this is an OAuth credential.
    pub fn is_oauth(&self) -> bool {
        matches!(self, Credential::OAuth(_))
    }

    /// Check if this is an API key credential.
    pub fn is_api_key(&self) -> bool {
        matches!(self, Credential::ApiKey(_))
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
