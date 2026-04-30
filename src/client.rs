//! Client + ClientBuilder implementation.
//!
//! Holds the [`reqwest::Client`], auth state, and shared internal helpers
//! (single-flight, cache, retry policy). Per-API method clusters are split
//! into sibling modules (`tenants.rs`, `indexes.rs`, ...) but each
//! delegates back here for the actual HTTP send.

use std::fmt;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_ENCODING, CONTENT_TYPE};
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::Write as _;
use tracing::{instrument, trace, warn};
use url::Url;

use crate::cache::TtlCache;
use crate::error::{ApiError, Error};
use crate::retry::{backoff_with_jitter, parse_retry_after};
use crate::singleflight::SingleFlight;
use crate::transport::{build_client, default_user_agent, TransportConfig};
use crate::types::SearchResponse;

/// Header used for tenant identification.
pub(crate) const TENANT_HEADER: &str = "X-Tenant-ID";
/// Header used for the API key.
pub(crate) const API_KEY_HEADER: &str = "X-API-Key";
/// Header to opt clients into receiving gzip in responses.
pub(crate) const ACCEPT_ENCODING: &str = "Accept-Encoding";
/// Threshold above which request bodies are gzipped before send.
pub(crate) const GZIP_THRESHOLD_BYTES: usize = 64 * 1024;

/// Pluggable metrics hook (only available with the `metrics` feature).
///
/// Each method is a no-op by default. Implementations receive every
/// observability point the SDK can emit.
#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
pub trait MetricsHook: Send + Sync + 'static {
    /// Called once per HTTP request the SDK initiates (after retries).
    fn on_request(&self, _method: &str, _path: &str, _status: Option<u16>, _elapsed: Duration) {}
    /// Called when the SDK retries a request because of a transient error.
    fn on_retry(&self, _attempt: u32, _reason: &str) {}
    /// Called when a singleflight call coalesces into an in-flight request.
    fn on_coalesced(&self, _key: &str) {}
    /// Called on each cache hit / miss.
    fn on_cache(&self, _hit: bool) {}
}

/// Public configuration snapshot passed into [`Client`].
///
/// Built via [`ClientBuilder`]. Cheap to clone — every field is small.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub(crate) base_url: Url,
    pub(crate) tenant_id: Option<String>,
    pub(crate) api_key: Option<RedactedApiKey>,
    pub(crate) timeout: Duration,
    pub(crate) connect_timeout: Duration,
    pub(crate) max_retries: u32,
    pub(crate) retry_base: Duration,
    pub(crate) retry_cap: Duration,
    pub(crate) https_only: bool,
    pub(crate) cache: Option<CacheConfig>,
    pub(crate) singleflight: bool,
    pub(crate) extra_headers: HeaderMap,
    pub(crate) user_agent: String,
    /// When `true`, request bodies above [`GZIP_THRESHOLD_BYTES`] are
    /// transparently compressed with gzip and sent with
    /// `Content-Encoding: gzip`. The graphann server does NOT decode
    /// gzipped request bodies — enabling this against a stock server
    /// surfaces as silent 400 "Invalid JSON body" errors on batches
    /// that cross the threshold. Default `false`. See
    /// [`ClientBuilder::compress_requests`].
    pub(crate) compress_requests: bool,
}

/// API key wrapped so [`Debug`] never leaks the secret value.
#[derive(Clone)]
pub(crate) struct RedactedApiKey(pub(crate) String);

impl fmt::Debug for RedactedApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"<redacted>\"")
    }
}

/// Optional response cache settings.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries before LRU eviction.
    pub capacity: NonZeroUsize,
    /// Time-to-live for each cached entry.
    pub ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            capacity: NonZeroUsize::new(256).expect("non-zero literal"),
            ttl: Duration::from_secs(60),
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            base_url: Url::parse("http://localhost:38888").expect("valid default url"),
            tenant_id: None,
            api_key: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_retries: 3,
            retry_base: Duration::from_millis(200),
            retry_cap: Duration::from_secs(20),
            https_only: false,
            cache: None,
            singleflight: true,
            extra_headers: HeaderMap::new(),
            user_agent: default_user_agent(),
            compress_requests: false,
        }
    }
}

/// Builder for [`Client`].
#[derive(Debug, Default)]
pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    /// Start with library defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the server base URL (e.g. `https://api.graphann.com`).
    pub fn base_url(mut self, url: impl AsRef<str>) -> Result<Self, Error> {
        let parsed = Url::parse(url.as_ref())?;
        self.config.base_url = parsed;
        Ok(self)
    }

    /// Set both the tenant id and the API key.
    pub fn api_key(mut self, tenant_id: impl Into<String>, api_key: impl Into<String>) -> Self {
        self.config.tenant_id = Some(tenant_id.into());
        self.config.api_key = Some(RedactedApiKey(api_key.into()));
        self
    }

    /// Set the per-request timeout (applies end-to-end including reads).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// Set the TCP connection timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.config.connect_timeout = timeout;
        self
    }

    /// Maximum number of retry attempts on retryable errors.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.config.max_retries = max_retries;
        self
    }

    /// Base for exponential backoff (default 200ms).
    pub fn retry_base(mut self, base: Duration) -> Self {
        self.config.retry_base = base;
        self
    }

    /// Cap for exponential backoff (default 20s).
    pub fn retry_cap(mut self, cap: Duration) -> Self {
        self.config.retry_cap = cap;
        self
    }

    /// Enable transparent gzip compression of request bodies above
    /// [`GZIP_THRESHOLD_BYTES`] (64 KiB). Default `false`.
    ///
    /// **Important:** the graphann HTTP server does not decode
    /// `Content-Encoding: gzip` on request bodies. Leaving this off is
    /// the safe choice; turn it on only when targeting a server you
    /// know decodes gzip (e.g. behind a custom proxy that decompresses
    /// before forwarding). Sending gzipped bodies to a stock graphann
    /// surfaces as silent 400 "Invalid JSON body" errors on batches
    /// that cross the threshold.
    pub fn compress_requests(mut self, enabled: bool) -> Self {
        self.config.compress_requests = enabled;
        self
    }

    /// Refuse to send requests over plain HTTP. Defaults to `false`.
    pub fn https_only(mut self, value: bool) -> Self {
        self.config.https_only = value;
        self
    }

    /// Enable response caching with the supplied [`CacheConfig`].
    pub fn cache(mut self, cfg: CacheConfig) -> Self {
        self.config.cache = Some(cfg);
        self
    }

    /// Toggle single-flight coalescing of concurrent identical search calls.
    pub fn singleflight(mut self, on: bool) -> Self {
        self.config.singleflight = on;
        self
    }

    /// Override the User-Agent. Use sparingly — the default identifies
    /// the SDK + rustc version + target arch.
    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.config.user_agent = ua.into();
        self
    }

    /// Add an extra outbound header (e.g. observability or canary tags).
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.config.extra_headers.insert(name, value);
        self
    }

    /// Finalize. Returns an [`Error::Builder`] when the URL or transport
    /// settings are invalid.
    pub fn build(self) -> Result<Client, Error> {
        let transport = TransportConfig {
            timeout: self.config.timeout,
            connect_timeout: self.config.connect_timeout,
            user_agent: self.config.user_agent.clone(),
            https_only: self.config.https_only,
            ..Default::default()
        };
        let http = build_client(&transport)?;

        let cache = self
            .config
            .cache
            .as_ref()
            .map(|c| TtlCache::new(c.capacity, c.ttl));
        let singleflight = if self.config.singleflight {
            Some(SingleFlight::<String, Arc<SearchResponse>>::new())
        } else {
            None
        };

        Ok(Client {
            inner: Arc::new(ClientInner {
                http,
                config: self.config,
                cache,
                singleflight,
                #[cfg(feature = "metrics")]
                metrics: parking_lot::RwLock::new(None),
            }),
        })
    }
}

/// Cheap-to-clone, send-everywhere handle to the SDK.
///
/// All state is wrapped in an [`Arc`] internally — `clone` is O(1).
#[derive(Clone)]
pub struct Client {
    pub(crate) inner: Arc<ClientInner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.inner.config.base_url.as_str())
            .field("tenant_id", &self.inner.config.tenant_id)
            .field("api_key", &self.inner.config.api_key)
            .field("timeout", &self.inner.config.timeout)
            .field("max_retries", &self.inner.config.max_retries)
            .finish_non_exhaustive()
    }
}

pub(crate) struct ClientInner {
    pub(crate) http: reqwest::Client,
    pub(crate) config: ClientConfig,
    pub(crate) cache: Option<TtlCache<String, Arc<SearchResponse>>>,
    pub(crate) singleflight: Option<SingleFlight<String, Arc<SearchResponse>>>,
    #[cfg(feature = "metrics")]
    pub(crate) metrics: parking_lot::RwLock<Option<Arc<dyn MetricsHook>>>,
}

impl Client {
    /// Construct a fresh builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Snapshot of the current effective configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.inner.config
    }

    /// Install a metrics hook (only available with the `metrics` feature).
    #[cfg(feature = "metrics")]
    #[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
    pub fn set_metrics_hook(&self, hook: Arc<dyn MetricsHook>) {
        let mut guard = self.inner.metrics.write();
        *guard = Some(hook);
    }

    /// Helper for callers — invalidates the response cache if any.
    pub fn invalidate_cache(&self) {
        if let Some(c) = &self.inner.cache {
            c.clear();
        }
    }

    /// Returns a new [`Client`] that targets `tenant_id` instead of the
    /// default tenant baked into the builder. Useful for callers that
    /// juggle multiple tenants (e.g. one default tenant plus per-campaign
    /// tenants) — building a fresh [`ClientBuilder`] for every call would
    /// also reset the response cache and singleflight group.
    ///
    /// The returned client shares the underlying HTTP transport (a single
    /// [`reqwest::Client`] connection pool) so this is cheap. Response
    /// cache and singleflight coalescing are NOT shared across tenants —
    /// the cloned client always starts with both disabled, since the
    /// cache key includes only the request path and not the tenant id.
    pub fn with_tenant_id(&self, tenant_id: impl Into<String>) -> Client {
        let mut config = self.inner.config.clone();
        config.tenant_id = Some(tenant_id.into());
        #[cfg(feature = "metrics")]
        let metrics = {
            let guard = self.inner.metrics.read();
            parking_lot::RwLock::new(guard.clone())
        };
        Client {
            inner: Arc::new(ClientInner {
                http: self.inner.http.clone(),
                config,
                cache: None,
                singleflight: None,
                #[cfg(feature = "metrics")]
                metrics,
            }),
        }
    }

    pub(crate) fn url(&self, path: &str) -> Result<Url, Error> {
        let trimmed = path.trim_start_matches('/');
        let mut base = self.inner.config.base_url.clone();
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }
        Ok(base.join(trimmed)?)
    }

    /// Build a request with auth headers applied.
    pub(crate) fn request(&self, method: Method, path: &str) -> Result<RequestBuilder, Error> {
        let url = self.url(path)?;
        let mut builder = self.inner.http.request(method, url);
        if let Some(tenant) = &self.inner.config.tenant_id {
            builder = builder.header(TENANT_HEADER, tenant);
        }
        if let Some(key) = &self.inner.config.api_key {
            builder = builder.header(API_KEY_HEADER, key.0.as_str());
        }
        for (k, v) in self.inner.config.extra_headers.iter() {
            builder = builder.header(k, v);
        }
        builder = builder.header(ACCEPT_ENCODING, "gzip");
        Ok(builder)
    }

    /// Send `body` as JSON, with automatic gzip when above the threshold.
    /// Returns the body as bytes for further decoding.
    #[instrument(skip(self, body), fields(method = %method, path = %path))]
    pub(crate) async fn send_json<B>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<Bytes, Error>
    where
        B: Serialize + ?Sized,
    {
        let mut attempt: u32 = 0;
        loop {
            let mut req = self.request(method.clone(), path)?;

            if let Some(b) = body {
                let raw = serde_json::to_vec(b)?;
                if self.inner.config.compress_requests && raw.len() >= GZIP_THRESHOLD_BYTES {
                    let mut encoder =
                        GzEncoder::new(Vec::with_capacity(raw.len() / 2), Compression::fast());
                    encoder.write_all(&raw)?;
                    let compressed = encoder.finish()?;
                    req = req
                        .header(CONTENT_TYPE, "application/json")
                        .header(CONTENT_ENCODING, "gzip")
                        .body(compressed);
                } else {
                    req = req.header(CONTENT_TYPE, "application/json").body(raw);
                }
            } else if matches!(
                method,
                Method::POST | Method::PUT | Method::PATCH | Method::DELETE
            ) {
                // Server middleware enforces Content-Type on mutating requests
                // even when the body is empty; satisfy it.
                req = req.header(CONTENT_TYPE, "application/json");
            }

            let started = std::time::Instant::now();
            let outcome = req.send().await;
            let elapsed = started.elapsed();
            trace!(?elapsed, attempt, "http response");

            match outcome {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() || status == StatusCode::NO_CONTENT {
                        let bytes = resp.bytes().await?;
                        #[cfg(feature = "metrics")]
                        self.report_request(method.as_str(), path, Some(status.as_u16()), elapsed);
                        return Ok(bytes);
                    }

                    let retry_after_header = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|h| h.to_str().ok())
                        .map(str::to_owned);
                    let body_bytes = resp.bytes().await.unwrap_or_default();
                    let api_err = parse_api_error(&body_bytes);
                    let err = classify_error(
                        status,
                        retry_after_header.as_deref(),
                        api_err.clone(),
                        &body_bytes,
                    );

                    if attempt < self.inner.config.max_retries && err.is_retryable() {
                        let delay = err.retry_after().unwrap_or_else(|| {
                            backoff_with_jitter(
                                attempt,
                                self.inner.config.retry_base,
                                self.inner.config.retry_cap,
                            )
                        });
                        warn!(
                            attempt,
                            ?delay,
                            status = status.as_u16(),
                            "retryable error, sleeping"
                        );
                        #[cfg(feature = "metrics")]
                        self.report_retry(attempt, &format!("status_{}", status.as_u16()));
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    #[cfg(feature = "metrics")]
                    self.report_request(method.as_str(), path, Some(status.as_u16()), elapsed);
                    return Err(err);
                }
                Err(e) => {
                    let err: Error = e.into();
                    if attempt < self.inner.config.max_retries && err.is_retryable() {
                        let delay = backoff_with_jitter(
                            attempt,
                            self.inner.config.retry_base,
                            self.inner.config.retry_cap,
                        );
                        warn!(attempt, ?delay, "network error, sleeping");
                        #[cfg(feature = "metrics")]
                        self.report_retry(attempt, "network");
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }
                    #[cfg(feature = "metrics")]
                    self.report_request(method.as_str(), path, None, elapsed);
                    return Err(err);
                }
            }
        }
    }

    /// Helper that decodes the body of a successful response into `T`.
    pub(crate) async fn request_json<B, T>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, Error>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let bytes = self.send_json(method, path, body).await?;
        if bytes.is_empty() {
            // Treat empty bodies as null — convenient for `()` returns.
            return Ok(serde_json::from_slice::<T>(b"null")?);
        }
        Ok(serde_json::from_slice::<T>(&bytes)?)
    }

    /// Variant for endpoints that return 204 No Content.
    pub(crate) async fn request_empty<B>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<(), Error>
    where
        B: Serialize + ?Sized,
    {
        let _ = self.send_json(method, path, body).await?;
        Ok(())
    }

    #[cfg(feature = "metrics")]
    fn report_request(&self, method: &str, path: &str, status: Option<u16>, elapsed: Duration) {
        if let Some(hook) = self.inner.metrics.read().clone() {
            hook.on_request(method, path, status, elapsed);
        }
    }

    #[cfg(feature = "metrics")]
    fn report_retry(&self, attempt: u32, reason: &str) {
        if let Some(hook) = self.inner.metrics.read().clone() {
            hook.on_retry(attempt, reason);
        }
    }

    /// Public health check (`GET /health`).
    pub async fn health(&self) -> Result<crate::types::Health, Error> {
        self.request_json(Method::GET, "health", Option::<&()>::None)
            .await
    }

    /// Public readiness check (`GET /ready`).
    pub async fn ready(&self) -> Result<crate::types::Ready, Error> {
        self.request_json(Method::GET, "ready", Option::<&()>::None)
            .await
    }

    /// Reports SDK version + best-effort server version.
    ///
    /// The server does not currently expose a dedicated `/version`
    /// endpoint, so the result's `server_version` is `None` until the
    /// server adds one. The SDK version is always populated from the
    /// crate's compile-time metadata.
    pub async fn version(&self) -> Result<crate::types::VersionInfo, Error> {
        let sdk_version = env!("CARGO_PKG_VERSION").to_string();
        // Probe `/version` — most servers don't have it and return 404.
        let server_version = match self
            .request_json::<(), serde_json::Value>(Method::GET, "version", Option::<&()>::None)
            .await
        {
            Ok(v) => v.get("version").and_then(|x| x.as_str()).map(str::to_owned),
            Err(Error::NotFound(_)) | Err(Error::Server { .. }) => None,
            Err(_) => None,
        };
        Ok(crate::types::VersionInfo {
            sdk_version,
            server_version,
        })
    }

    pub(crate) fn cache(&self) -> Option<&TtlCache<String, Arc<SearchResponse>>> {
        self.inner.cache.as_ref()
    }

    /// Internal helper used by per-namespace methods to fetch the
    /// builder-supplied tenant id, returning a typed builder error when
    /// none was configured.
    pub(crate) fn require_tenant(&self) -> Result<&str, Error> {
        self.config()
            .tenant_id
            .as_deref()
            .ok_or_else(|| Error::Builder("no tenant id configured on client".into()))
    }

    pub(crate) fn singleflight(&self) -> Option<&SingleFlight<String, Arc<SearchResponse>>> {
        self.inner.singleflight.as_ref()
    }
}

/// Translate the parsed body + status into the public [`Error`] hierarchy.
fn classify_error(
    status: StatusCode,
    retry_after: Option<&str>,
    api_err: Option<ApiError>,
    body: &[u8],
) -> Error {
    let retry_after_dur = retry_after.and_then(parse_retry_after);
    let message = api_err
        .as_ref()
        .map(|e| e.message.clone())
        .unwrap_or_else(|| trimmed_body(body));
    let code = api_err.as_ref().map(|e| e.code.clone());

    match status {
        StatusCode::UNAUTHORIZED => Error::Auth(message),
        StatusCode::FORBIDDEN => Error::Authorization(message),
        StatusCode::NOT_FOUND => Error::NotFound(message),
        StatusCode::CONFLICT => Error::Conflict(message),
        StatusCode::PAYLOAD_TOO_LARGE => Error::PayloadTooLarge(message),
        StatusCode::TOO_MANY_REQUESTS => Error::RateLimit {
            retry_after: retry_after_dur,
            message: Some(message),
        },
        StatusCode::SERVICE_UNAVAILABLE => Error::ServiceUnavailable {
            code,
            message,
            retry_after: retry_after_dur,
        },
        s if s.is_server_error() => Error::Server {
            status: s.as_u16(),
            code,
            body: trimmed_body(body),
        },
        s => Error::Server {
            status: s.as_u16(),
            code,
            body: trimmed_body(body),
        },
    }
}

fn trimmed_body(body: &[u8]) -> String {
    const MAX: usize = 4096;
    let raw = String::from_utf8_lossy(body);
    if raw.len() > MAX {
        format!("{}...(truncated)", &raw[..MAX])
    } else {
        raw.into_owned()
    }
}

fn parse_api_error(body: &[u8]) -> Option<ApiError> {
    #[derive(serde::Deserialize)]
    struct Envelope {
        error: ApiError,
    }
    serde_json::from_slice::<Envelope>(body)
        .ok()
        .map(|e| e.error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_api_key() {
        let client = ClientBuilder::new()
            .base_url("https://api.example.test")
            .unwrap()
            .api_key("t_x", "ak_secret")
            .build()
            .unwrap();
        let dbg = format!("{client:?}");
        assert!(dbg.contains("<redacted>"));
        assert!(!dbg.contains("ak_secret"));
    }

    #[test]
    fn url_join_handles_trailing_slash() {
        let client = ClientBuilder::new()
            .base_url("https://api.example.test/v1")
            .unwrap()
            .build()
            .unwrap();
        let url = client.url("health").unwrap();
        assert_eq!(url.as_str(), "https://api.example.test/v1/health");
    }
}
