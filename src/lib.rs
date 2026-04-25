//! Official Rust SDK for the [GraphANN](https://graphann.com) vector database.
//!
//! GraphANN is a storage-efficient vector database built on the LEANN
//! (Low-storage Embedding Approximate Nearest Neighbor) algorithm. This crate
//! provides an idiomatic, async-by-default client for every public HTTP route
//! the server exposes.
//!
//! # Quick start
//!
//! ```no_run
//! use std::time::Duration;
//! use graphann::{ClientBuilder, SearchRequest};
//!
//! # async fn run() -> Result<(), graphann::Error> {
//! let client = ClientBuilder::new()
//!     .base_url("https://api.graphann.com")?
//!     .api_key("t_xyz789", "ak_demo")
//!     .timeout(Duration::from_secs(30))
//!     .max_retries(3)
//!     .build()?;
//!
//! let _health = client.health().await?;
//!
//! let req = SearchRequest {
//!     query: Some("hello".into()),
//!     k: 10,
//!     ..Default::default()
//! };
//! let _results = client.search("i_abc123", req).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Cargo features
//!
//! - `default = ["rustls"]` — enable rustls TLS (no OpenSSL dependency).
//! - `rustls` — TLS via rustls.
//! - `native-tls` — alternative TLS via the platform's native stack.
//! - `blocking` — synchronous wrapper alongside the async API.
//! - `metrics` — pluggable metrics hook.
//!
//! # Logging
//!
//! Internal events are emitted via the [`tracing`] crate. The consumer is
//! responsible for installing a subscriber (e.g. `tracing_subscriber::fmt`).
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![warn(rustdoc::broken_intra_doc_links)]
#![doc(test(no_crate_inject))]
// Compile the README's code blocks under `cargo test --doc`. Pinned via
// `cfg(doctest)` so the README does not double as the crate's main page
// (the module docstring above is the canonical front door).
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctest {}

mod cache;
mod client;
mod error;
mod pagination;
mod retry;
mod singleflight;
mod transport;

pub mod apikey;
pub mod cluster;
pub mod documents;
pub mod health;
pub mod indexes;
pub mod jobs;
pub mod org;
pub mod search;
pub mod settings;
pub mod tenants;
pub mod types;

#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub mod blocking;

#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
pub use crate::client::MetricsHook;

pub use crate::client::{CacheConfig, Client, ClientBuilder, ClientConfig};
pub use crate::error::{ApiError, Error, ErrorCode};
pub use crate::pagination::{Page, PageStream};
pub use crate::types::*;

#[cfg(feature = "blocking")]
#[cfg_attr(docsrs, doc(cfg(feature = "blocking")))]
pub use crate::blocking::BlockingClient;
