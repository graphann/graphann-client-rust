//! Error types returned by the GraphANN client.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level error returned from every fallible client method.
///
/// The hierarchy distinguishes between server-mapped HTTP statuses
/// (so callers can branch on `Error::NotFound` without parsing a code)
/// and transport-level failures wrapped from [`reqwest`] / [`serde_json`].
#[derive(Debug, Error)]
pub enum Error {
    /// 401 Unauthorized — credentials missing or invalid.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// 403 Forbidden — credentials valid but lacking permission.
    #[error("authorization failed: {0}")]
    Authorization(String),

    /// 404 Not Found.
    #[error("resource not found: {0}")]
    NotFound(String),

    /// 409 Conflict.
    #[error("conflict: {0}")]
    Conflict(String),

    /// 413 Payload Too Large.
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),

    /// 429 Too Many Requests. Carries a parsed `Retry-After` duration when
    /// the server provided one.
    #[error("rate limited{}", retry_after.map(|d| format!(", retry after {:?}", d)).unwrap_or_default())]
    RateLimit {
        /// Suggested back-off interval as parsed from `Retry-After`.
        retry_after: Option<Duration>,
        /// Optional textual message returned by the server.
        message: Option<String>,
    },

    /// 503 Service Unavailable, including index-not-ready and quota errors.
    #[error("service unavailable: {message}")]
    ServiceUnavailable {
        /// Stable error code from the server (e.g. `index_not_ready`).
        code: Option<String>,
        /// Human-readable message.
        message: String,
        /// Suggested back-off if the server set `Retry-After`.
        retry_after: Option<Duration>,
    },

    /// 4xx/5xx not otherwise classified.
    #[error("server error {status}: {body}")]
    Server {
        /// HTTP status code.
        status: u16,
        /// Stable error code from the server, when present.
        code: Option<String>,
        /// Raw / decoded body — already truncated to a safe length.
        body: String,
    },

    /// Network-level error wrapping a [`reqwest::Error`].
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON encode/decode failure.
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    /// Local I/O failure (e.g. preparing a request body).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// URL parsing failure.
    #[error("invalid url: {0}")]
    Url(#[from] url::ParseError),

    /// Misuse during [`crate::ClientBuilder::build`].
    #[error("client builder error: {0}")]
    Builder(String),

    /// Catch-all for anything that does not fit a more specific bucket.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Returns `true` for transport- or status-level errors that are safe to
    /// retry (network / 429 / 503).
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::RateLimit { .. } | Error::ServiceUnavailable { .. } | Error::Network(_)
        )
    }

    /// Suggested back-off duration parsed from a `Retry-After` header, if any.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Error::RateLimit { retry_after, .. } => *retry_after,
            Error::ServiceUnavailable { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

/// JSON error body returned by the GraphANN server.
///
/// The server wraps all error responses as `{"error": {"code", "message"}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Stable, machine-readable error code (e.g. `not_found`, `validation_error`).
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Optional structured details supplied by some endpoints (e.g. job id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Stable error codes the server emits. Mirrors `internal/server/errors.go`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Internal error.
    Internal,
    /// Bad request body or parameters.
    BadRequest,
    /// Authentication required.
    Unauthorized,
    /// Caller authenticated but lacks permission.
    Forbidden,
    /// Resource not found.
    NotFound,
    /// Conflicting state.
    Conflict,
    /// Resource quota exceeded.
    QuotaExceeded,
    /// Rate limit exceeded.
    RateLimited,
    /// Validation of a structured field failed.
    Validation,
    /// Index not ready for search.
    IndexNotReady,
    /// Index currently being built.
    IndexBuilding,
    /// Catch-all 503.
    ServiceUnavailable,
    /// Body too large.
    PayloadTooLarge,
    /// Endpoint exists but the operation is not implemented.
    NotImplemented,
    /// Unrecognised code (forward-compatible escape hatch).
    Other,
}

impl ErrorCode {
    /// Parse the wire string returned in `{"error": {"code"}}`.
    ///
    /// We deliberately don't implement `std::str::FromStr` — that trait
    /// requires a fallible signature and `Other` is a sound fallback for
    /// any unknown string, so the infallible mapping fits better as an
    /// inherent associated function.
    pub fn parse(s: &str) -> Self {
        match s {
            "internal_error" | "internal" => ErrorCode::Internal,
            "bad_request" => ErrorCode::BadRequest,
            "unauthorized" => ErrorCode::Unauthorized,
            "forbidden" => ErrorCode::Forbidden,
            "not_found" => ErrorCode::NotFound,
            "conflict" => ErrorCode::Conflict,
            "quota_exceeded" => ErrorCode::QuotaExceeded,
            "rate_limited" => ErrorCode::RateLimited,
            "validation" | "validation_error" => ErrorCode::Validation,
            "index_not_ready" => ErrorCode::IndexNotReady,
            "index_building" => ErrorCode::IndexBuilding,
            "service_unavailable" => ErrorCode::ServiceUnavailable,
            "payload_too_large" => ErrorCode::PayloadTooLarge,
            "not_implemented" => ErrorCode::NotImplemented,
            _ => ErrorCode::Other,
        }
    }
}
