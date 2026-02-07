//! Network configuration for proxy, TLS, certificate, and connection pool settings.

use std::env;
use std::path::PathBuf;
use std::time::Duration;

/// Connection pool configuration.
#[derive(Clone, Debug)]
pub struct PoolConfig {
    pub idle_timeout: Duration,
    pub max_idle_per_host: usize,
    pub tcp_keepalive: Option<Duration>,
    pub http2_keep_alive: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(90),
            max_idle_per_host: 32,
            tcp_keepalive: Some(Duration::from_secs(60)),
            http2_keep_alive: Some(Duration::from_secs(30)),
        }
    }
}

impl PoolConfig {
    pub fn minimal() -> Self {
        Self {
            idle_timeout: Duration::from_secs(30),
            max_idle_per_host: 2,
            tcp_keepalive: None,
            http2_keep_alive: None,
        }
    }
}

/// Network configuration for HTTP client.
#[derive(Clone, Debug, Default)]
pub struct HttpNetworkConfig {
    /// Proxy configuration
    pub proxy: Option<ProxyConfig>,
    /// Custom CA certificate file path
    pub ca_cert: Option<PathBuf>,
    /// Client certificate for mTLS
    pub client_cert: Option<ClientCertConfig>,
    /// Connection pool settings
    pub pool: Option<PoolConfig>,
}

/// Proxy server configuration.
#[derive(Clone, Debug)]
pub struct ProxyConfig {
    /// HTTPS proxy URL
    pub https: Option<String>,
    /// HTTP proxy URL
    pub http: Option<String>,
    /// No-proxy patterns (space or comma separated)
    pub no_proxy: Vec<String>,
}

/// Client certificate configuration for mTLS.
#[derive(Clone, Debug)]
pub struct ClientCertConfig {
    /// Path to client certificate (PEM)
    pub cert_path: PathBuf,
    /// Path to client private key (PEM)
    pub key_path: PathBuf,
    /// Optional passphrase for encrypted key
    pub key_passphrase: Option<String>,
}

impl HttpNetworkConfig {
    /// Create from environment variables.
    pub fn from_env() -> Self {
        Self {
            proxy: ProxyConfig::from_env(),
            ca_cert: env::var("SSL_CERT_FILE")
                .ok()
                .or_else(|| env::var("REQUESTS_CA_BUNDLE").ok())
                .map(PathBuf::from),
            client_cert: ClientCertConfig::from_env(),
            pool: None,
        }
    }

    /// Set proxy configuration.
    pub fn proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Set CA certificate path.
    pub fn ca_cert(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_cert = Some(path.into());
        self
    }

    /// Set client certificate for mTLS.
    pub fn client_cert(mut self, cert: ClientCertConfig) -> Self {
        self.client_cert = Some(cert);
        self
    }

    /// Set connection pool configuration.
    pub fn pool(mut self, pool: PoolConfig) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Check if any network configuration is set.
    pub fn is_configured(&self) -> bool {
        self.proxy.is_some()
            || self.ca_cert.is_some()
            || self.client_cert.is_some()
            || self.pool.is_some()
    }

    /// Apply configuration to reqwest ClientBuilder.
    pub async fn apply_to_builder(
        &self,
        mut builder: reqwest::ClientBuilder,
    ) -> Result<reqwest::ClientBuilder, std::io::Error> {
        if let Some(ref proxy) = self.proxy {
            builder = proxy.apply_to_builder(builder)?;
        }

        if let Some(ref ca_path) = self.ca_cert {
            let cert_data = tokio::fs::read(ca_path).await?;
            if let Ok(cert) = reqwest::Certificate::from_pem(&cert_data) {
                builder = builder.add_root_certificate(cert);
            }
        }

        if let Some(ref client_cert) = self.client_cert {
            builder = client_cert.apply_to_builder(builder).await?;
        }

        if let Some(ref pool) = self.pool {
            builder = builder
                .pool_idle_timeout(pool.idle_timeout)
                .pool_max_idle_per_host(pool.max_idle_per_host);

            if let Some(keepalive) = pool.tcp_keepalive {
                builder = builder.tcp_keepalive(keepalive);
            }

            if let Some(interval) = pool.http2_keep_alive {
                builder = builder
                    .http2_keep_alive_interval(interval)
                    .http2_keep_alive_while_idle(true);
            }
        }

        Ok(builder)
    }
}

impl ProxyConfig {
    /// Create from environment variables.
    pub fn from_env() -> Option<Self> {
        let https = env::var("HTTPS_PROXY")
            .ok()
            .or_else(|| env::var("https_proxy").ok());
        let http = env::var("HTTP_PROXY")
            .ok()
            .or_else(|| env::var("http_proxy").ok());

        if https.is_none() && http.is_none() {
            return None;
        }

        let no_proxy = env::var("NO_PROXY")
            .ok()
            .or_else(|| env::var("no_proxy").ok())
            .map(|s| {
                s.split([',', ' '])
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            https,
            http,
            no_proxy,
        })
    }

    /// Create with HTTPS proxy.
    pub fn https(url: impl Into<String>) -> Self {
        Self {
            https: Some(url.into()),
            http: None,
            no_proxy: Vec::new(),
        }
    }

    /// Add HTTP proxy.
    pub fn http(mut self, url: impl Into<String>) -> Self {
        self.http = Some(url.into());
        self
    }

    /// Add no-proxy patterns.
    pub fn no_proxy(mut self, patterns: impl IntoIterator<Item = String>) -> Self {
        self.no_proxy.extend(patterns);
        self
    }

    /// Apply to reqwest ClientBuilder.
    pub fn apply_to_builder(
        &self,
        mut builder: reqwest::ClientBuilder,
    ) -> Result<reqwest::ClientBuilder, std::io::Error> {
        if let Some(ref https_url) = self.https
            && let Ok(proxy) = reqwest::Proxy::https(https_url)
        {
            builder = builder.proxy(proxy);
        }
        if let Some(ref http_url) = self.http
            && let Ok(proxy) = reqwest::Proxy::http(http_url)
        {
            builder = builder.proxy(proxy);
        }
        // Note: no_proxy is typically handled by the proxy itself or system config
        Ok(builder)
    }
}

impl ClientCertConfig {
    /// Create from environment variables.
    pub fn from_env() -> Option<Self> {
        let cert_path = env::var("CLAUDE_CODE_CLIENT_CERT").ok()?;
        let key_path = env::var("CLAUDE_CODE_CLIENT_KEY").ok()?;
        let key_passphrase = env::var("CLAUDE_CODE_CLIENT_KEY_PASSPHRASE").ok();

        Some(Self {
            cert_path: PathBuf::from(cert_path),
            key_path: PathBuf::from(key_path),
            key_passphrase,
        })
    }

    /// Create with certificate paths.
    pub fn new(cert_path: impl Into<PathBuf>, key_path: impl Into<PathBuf>) -> Self {
        Self {
            cert_path: cert_path.into(),
            key_path: key_path.into(),
            key_passphrase: None,
        }
    }

    /// Set key passphrase.
    pub fn passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.key_passphrase = Some(passphrase.into());
        self
    }

    /// Apply to reqwest ClientBuilder.
    pub async fn apply_to_builder(
        &self,
        builder: reqwest::ClientBuilder,
    ) -> Result<reqwest::ClientBuilder, std::io::Error> {
        let cert_data = tokio::fs::read(&self.cert_path).await?;
        let key_data = tokio::fs::read(&self.key_path).await?;

        let mut pem_data = cert_data;
        pem_data.extend_from_slice(b"\n");
        pem_data.extend_from_slice(&key_data);

        let identity = reqwest::Identity::from_pem(&pem_data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(builder.identity(identity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_config_builder() {
        let proxy = ProxyConfig::https("https://proxy.example.com:8080")
            .http("http://proxy.example.com:8080")
            .no_proxy(vec!["localhost".to_string(), "*.internal".to_string()]);

        assert!(proxy.https.is_some());
        assert!(proxy.http.is_some());
        assert_eq!(proxy.no_proxy.len(), 2);
    }

    #[test]
    fn test_network_config_builder() {
        let config = HttpNetworkConfig::default()
            .proxy(ProxyConfig::https("https://proxy.com"))
            .ca_cert("/path/to/ca.pem");

        assert!(config.proxy.is_some());
        assert!(config.ca_cert.is_some());
        assert!(config.is_configured());
    }
}
