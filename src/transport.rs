//! Reqwest client builder with hardened defaults.

use std::time::Duration;

use reqwest::Client as ReqwestClient;

use crate::error::Error;

/// Hardened transport settings sourced from the public [`crate::ClientConfig`].
#[derive(Debug, Clone)]
pub(crate) struct TransportConfig {
    pub(crate) timeout: Duration,
    pub(crate) connect_timeout: Duration,
    pub(crate) pool_idle_timeout: Option<Duration>,
    pub(crate) pool_max_idle_per_host: usize,
    pub(crate) tcp_nodelay: bool,
    pub(crate) https_only: bool,
    pub(crate) user_agent: String,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            pool_idle_timeout: Some(Duration::from_secs(90)),
            pool_max_idle_per_host: 32,
            tcp_nodelay: true,
            https_only: false,
            user_agent: default_user_agent(),
        }
    }
}

/// Default User-Agent string. Format documented in the SDK README:
/// `graphann-rust/<version> (rustc/<rustc>; <os>/<arch>)`.
pub(crate) fn default_user_agent() -> String {
    let pkg_version = env!("CARGO_PKG_VERSION");
    let rustc_version = option_env!("CARGO_RUSTC_VERSION").unwrap_or("unknown");
    format!(
        "graphann-rust/{} (rustc/{}; {}/{})",
        pkg_version,
        rustc_version,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Build the underlying [`reqwest::Client`].
pub(crate) fn build_client(cfg: &TransportConfig) -> Result<ReqwestClient, Error> {
    let mut builder = ReqwestClient::builder()
        .timeout(cfg.timeout)
        .connect_timeout(cfg.connect_timeout)
        .pool_max_idle_per_host(cfg.pool_max_idle_per_host)
        .tcp_nodelay(cfg.tcp_nodelay)
        .https_only(cfg.https_only)
        .user_agent(cfg.user_agent.clone())
        .gzip(true);

    if let Some(idle) = cfg.pool_idle_timeout {
        builder = builder.pool_idle_timeout(idle);
    }

    builder
        .build()
        .map_err(|e| Error::Builder(format!("reqwest build failed: {e}")))
}
