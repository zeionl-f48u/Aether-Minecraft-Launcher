//! HTTP client and networking utilities.
//!
//! Provides a configurable HTTP client with automatic retry, mirror fallback,
//! and optional proxy support (disabled by default).

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use log::{debug, error, warn};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::{Client, ClientBuilder, Response, StatusCode};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use thiserror::Error;

// ============================================================================
//  Error Handling
// ============================================================================

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP status error: {0}")]
    Status(StatusCode),

    #[error("All mirrors failed")]
    AllMirrorsFailed,

    #[error("Response body too large (limit: {limit} bytes)")]
    BodyTooLarge { limit: u64 },

    #[error("Request cancelled")]
    Cancelled,
}

pub type HttpResult<T> = Result<T, HttpError>;

// ============================================================================
//  Proxy Configuration
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyType {
    Http,
    Https,
    Socks5,
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub proxy_type: ProxyType,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    pub fn new(proxy_type: ProxyType, host: impl Into<String>, port: u16) -> Self {
        Self {
            proxy_type,
            host: host.into(),
            port,
            username: None,
            password: None,
        }
    }

    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    pub fn to_reqwest_proxy(&self) -> HttpResult<reqwest::Proxy> {
        let url = match self.proxy_type {
            ProxyType::Http => format!("http://{}:{}", self.host, self.port),
            ProxyType::Https => format!("https://{}:{}", self.host, self.port),
            ProxyType::Socks5 => format!("socks5://{}:{}", self.host, self.port),
        };

        let mut proxy = reqwest::Proxy::all(&url)
            .map_err(|e| HttpError::InvalidUrl(format!("Invalid proxy URL: {}", e)))?;

        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            proxy = proxy.basic_auth(user, pass);
        }

        Ok(proxy)
    }

    /// Detect system proxy from environment variables (HTTP_PROXY, HTTPS_PROXY, ALL_PROXY).
    pub fn from_env() -> Option<Self> {
        let var = std::env::var("HTTPS_PROXY")
            .or_else(|_| std::env::var("https_proxy"))
            .or_else(|_| std::env::var("HTTP_PROXY"))
            .or_else(|_| std::env::var("http_proxy"))
            .or_else(|_| std::env::var("ALL_PROXY"))
            .or_else(|_| std::env::var("all_proxy"))
            .ok()?;

        if let Ok(url) = url::Url::parse(&var) {
            let scheme = url.scheme();
            let host = url.host_str()?.to_string();
            let port = url.port().unwrap_or(1080);

            let proxy_type = match scheme {
                "http" => ProxyType::Http,
                "https" => ProxyType::Https,
                "socks5" => ProxyType::Socks5,
                _ => return None,
            };

            let mut config = ProxyConfig::new(proxy_type, host, port);
            if !url.username().is_empty() {
                config = config.with_auth(url.username(), url.password().unwrap_or(""));
            }
            Some(config)
        } else {
            None
        }
    }
}

// ============================================================================
//  Configuration & Client
// ============================================================================

#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub user_agent: String,
    pub connect_timeout: Duration,
    pub total_timeout: Duration,
    pub proxy: Option<ProxyConfig>,        // None = disabled
    pub retry_count: usize,
    pub base_retry_delay: Duration,
    pub max_response_size: Option<u64>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            user_agent: format!(
                "aether-minecraft-launcher/{}",
                std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "dev".into())
            ),
            connect_timeout: Duration::from_secs(10),
            total_timeout: Duration::from_secs(120),
            proxy: None,  // 默认不启用代理
            retry_count: 5,
            base_retry_delay: Duration::from_millis(500),
            max_response_size: Some(1024 * 1024 * 1024), // 1GB
        }
    }
}

impl HttpClientConfig {
    /// Create a config with automatic proxy detection from environment variables.
    pub fn with_auto_proxy() -> Self {
        Self {
            proxy: ProxyConfig::from_env(),
            ..Default::default()
        }
    }

    /// Set a specific proxy configuration.
    pub fn with_proxy(mut self, proxy: ProxyConfig) -> Self {
        self.proxy = Some(proxy);
        self
    }
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client,
    config: HttpClientConfig,
}

impl HttpClient {
    pub fn new(config: HttpClientConfig) -> HttpResult<Self> {
        let mut headers = HeaderMap::new();
        if let Ok(ua) = HeaderValue::from_str(&config.user_agent) {
            headers.insert(USER_AGENT, ua);
        }

        let mut builder = ClientBuilder::new()
            .connect_timeout(config.connect_timeout)
            .timeout(config.total_timeout)
            .default_headers(headers);

        if let Some(proxy_config) = &config.proxy {
            let proxy = proxy_config.to_reqwest_proxy()?;
            builder = builder.proxy(proxy);
        }

        Ok(Self {
            inner: builder.build()?,
            config,
        })
    }

    pub fn default() -> Self {
        Self::new(HttpClientConfig::default()).expect("Failed to build default HTTP client")
    }

    /// Create a client with automatic proxy detection.
    pub fn with_auto_proxy() -> Self {
        Self::new(HttpClientConfig::with_auto_proxy())
            .expect("Failed to build HTTP client with auto proxy")
    }

    /// Internal GET with retry and optional cancellation.
    async fn get_with_retry_inner(
        &self,
        url: &str,
        cancel_token: Option<&CancellationToken>,
    ) -> HttpResult<Response> {
        let mut attempt = 0;
        let max_attempts = self.config.retry_count.max(1);
        let base_delay = self.config.base_retry_delay;

        loop {
            attempt += 1;
            debug!("GET attempt {}/{}: {}", attempt, max_attempts, url);

            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(HttpError::Cancelled);
                }
            }

            let result = self.inner.get(url).send().await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp);
                    }

                    let should_retry = if status.is_server_error() {
                        true
                    } else if status == StatusCode::TOO_MANY_REQUESTS {
                        true
                    } else if status.is_client_error() {
                        false
                    } else {
                        false
                    };

                    if should_retry && attempt < max_attempts {
                        let delay = Self::calc_delay(base_delay, attempt);
                        warn!("GET {} returned {} retrying in {:?}", url, status, delay);
                        sleep(delay).await;
                        continue;
                    } else {
                        return Err(HttpError::Status(status));
                    }
                }
                Err(e) => {
                    if e.is_timeout() || e.is_connect() || e.is_request() {
                        if attempt < max_attempts {
                            let delay = Self::calc_delay(base_delay, attempt);
                            warn!("GET {} error: {} retrying in {:?}", url, e, delay);
                            sleep(delay).await;
                            continue;
                        } else {
                            return Err(e.into());
                        }
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }
    }

    fn calc_delay(base: Duration, attempt: usize) -> Duration {
        let exp = 2u32.pow(attempt as u32 - 1);
        let base_ms = base.as_millis() as u64;
        let delay_ms = base_ms * exp as u64;
        let jitter = rand::random::<u64>() % (delay_ms / 2);
        Duration::from_millis(delay_ms + jitter)
    }

    pub async fn get_with_retry(&self, url: &str) -> HttpResult<Response> {
        self.get_with_retry_inner(url, None).await
    }

    pub async fn get_with_retry_cancel(
        &self,
        url: &str,
        cancel_token: &CancellationToken,
    ) -> HttpResult<Response> {
        self.get_with_retry_inner(url, Some(cancel_token)).await
    }

    async fn download_inner(
        &self,
        uris: &[String],
        dest: &Path,
        cancel_token: Option<&CancellationToken>,
        progress_callback: Option<impl Fn(u64, u64) + Send>,
    ) -> HttpResult<()> {
        if uris.is_empty() {
            return Err(HttpError::InvalidUrl("No mirrors provided".into()));
        }

        let mut last_err = None;
        for (idx, uri) in uris.iter().enumerate() {
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(HttpError::Cancelled);
                }
            }

            debug!("Download attempt {} from {}", idx + 1, uri);
            let resp = match self.get_with_retry_inner(uri, cancel_token).await {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to fetch from {}: {}", uri, e);
                    last_err = Some(e);
                    continue;
                }
            };

            let content_len = resp.content_length();
            if let Some(limit) = self.config.max_response_size {
                if let Some(len) = content_len {
                    if len > limit {
                        return Err(HttpError::BodyTooLarge { limit });
                    }
                }
            }

            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            let mut file = File::create(dest).await?;
            let mut stream = resp.bytes_stream();
            use futures_util::StreamExt;
            let mut downloaded = 0u64;

            while let Some(chunk_result) = stream.next().await {
                if let Some(token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(HttpError::Cancelled);
                    }
                }

                let chunk = chunk_result?;
                downloaded += chunk.len() as u64;

                if let Some(limit) = self.config.max_response_size {
                    if downloaded > limit {
                        return Err(HttpError::BodyTooLarge { limit });
                    }
                }

                file.write_all(&chunk).await?;

                if let Some(cb) = &progress_callback {
                    cb(downloaded, content_len.unwrap_or(0));
                }
            }

            file.flush().await?;
            return Ok(());
        }

        Err(last_err.unwrap_or(HttpError::AllMirrorsFailed))
    }

    pub async fn download(&self, uris: &[String], dest: &Path) -> HttpResult<()> {
        self.download_inner(uris, dest, None, Option::<fn(u64, u64)>::None)
            .await
    }

    pub async fn download_with_cancel(
        &self,
        uris: &[String],
        dest: &Path,
        cancel_token: &CancellationToken,
    ) -> HttpResult<()> {
        self.download_inner(uris, dest, Some(cancel_token), Option::<fn(u64, u64)>::None)
            .await
    }

    pub async fn download_with_progress<F>(
        &self,
        uris: &[String],
        dest: &Path,
        progress: F,
    ) -> HttpResult<()>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        self.download_inner(uris, dest, None, Some(progress)).await
    }

    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> HttpResult<T> {
        let resp = self.get_with_retry(url).await?;
        let bytes = resp.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn get_bytes(&self, url: &str) -> HttpResult<Vec<u8>> {
        let resp = self.get_with_retry(url).await?;
        let size = resp.content_length().unwrap_or(0);
        if let Some(limit) = self.config.max_response_size {
            if size > limit {
                return Err(HttpError::BodyTooLarge { limit });
            }
        }
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn get_string(&self, url: &str) -> HttpResult<String> {
        let resp = self.get_with_retry(url).await?;
        let size = resp.content_length().unwrap_or(0);
        if let Some(limit) = self.config.max_response_size {
            if size > limit {
                return Err(HttpError::BodyTooLarge { limit });
            }
        }
        Ok(resp.text().await?)
    }

    pub async fn post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &T,
    ) -> HttpResult<R> {
        let resp = self
            .inner
            .post(url)
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            return Err(HttpError::Status(status));
        }
        let bytes = resp.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }

    pub fn config(&self) -> &HttpClientConfig {
        &self.config
    }
}

// ============================================================================
//  Global Default Client
// ============================================================================

pub static GLOBAL_CLIENT: once_cell::sync::Lazy<HttpClient> =
    once_cell::sync::Lazy::new(HttpClient::default);

// ============================================================================
//  Legacy Compatibility Functions
// ============================================================================

pub async fn retry_future<F, T, E>(fut: F) -> Result<T, anyhow::Error>
where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>> + Send>>,
    E: std::error::Error + Send + Sync + 'static,
{
    let config = &GLOBAL_CLIENT.config;
    let mut attempt = 0;
    let max_attempts = config.retry_count.max(1);
    let base_delay = config.base_retry_delay;

    loop {
        attempt += 1;
        match fut().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= max_attempts {
                    return Err(anyhow::anyhow!(e));
                }
                let delay = HttpClient::calc_delay(base_delay, attempt);
                sleep(delay).await;
            }
        }
    }
}

pub async fn download(uris: &[String], dest: &Path, _parallel: usize) -> anyhow::Result<()> {
    GLOBAL_CLIENT.download(uris, dest).await?;
    Ok(())
}

pub async fn retry_get_json<T: serde::de::DeserializeOwned>(url: &str) -> anyhow::Result<T> {
    Ok(GLOBAL_CLIENT.get_json(url).await?)
}

pub async fn retry_get_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    Ok(GLOBAL_CLIENT.get_bytes(url).await?)
}

pub async fn retry_get_string(url: &str) -> anyhow::Result<String> {
    Ok(GLOBAL_CLIENT.get_string(url).await?)
}

// ============================================================================
//  Sub-module: no_retry
// ============================================================================

pub mod no_retry {
    use super::*;

    pub async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> HttpResult<T> {
        let resp = GLOBAL_CLIENT.inner.get(url).send().await?;
        let bytes = resp.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub async fn get_bytes(url: &str) -> HttpResult<Vec<u8>> {
        let resp = GLOBAL_CLIENT.inner.get(url).send().await?;
        Ok(resp.bytes().await?.to_vec())
    }

    pub async fn get_string(url: &str) -> HttpResult<String> {
        let resp = GLOBAL_CLIENT.inner.get(url).send().await?;
        Ok(resp.text().await?)
    }
}

// ============================================================================
//  Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_string() {
        let client = HttpClient::default();
        let result = client.get_string("https://httpbin.org/get").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_download_mirrors() {
        let client = HttpClient::default();
        let uris = vec![
            "https://httpbin.org/status/500".to_string(),
            "https://httpbin.org/bytes/10".to_string(),
        ];
        let dest = PathBuf::from("/tmp/test_download.bin");
        let result = client.download(&uris, &dest).await;
        assert!(result.is_ok());
        let _ = tokio::fs::remove_file(&dest).await;
    }

    #[test]
    fn test_proxy_from_env() {
        std::env::set_var("HTTP_PROXY", "http://user:pass@proxy.example:8080");
        let proxy = ProxyConfig::from_env();
        assert!(proxy.is_some());
        let p = proxy.unwrap();
        assert_eq!(p.host, "proxy.example");
        assert_eq!(p.port, 8080);
        assert_eq!(p.username, Some("user".to_string()));
        assert_eq!(p.password, Some("pass".to_string()));
    }

    #[test]
    fn test_default_proxy_disabled() {
        let config = HttpClientConfig::default();
        assert!(config.proxy.is_none());
    }

    #[test]
    fn test_with_auto_proxy() {
        // Set a proxy env var
        std::env::set_var("HTTP_PROXY", "http://proxy.test:8080");
        let config = HttpClientConfig::with_auto_proxy();
        assert!(config.proxy.is_some());
    }
}