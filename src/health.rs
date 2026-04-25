//! Liveness and readiness helpers.
//!
//! The health-probe methods live on [`crate::Client`] directly. This
//! module exists so `cargo doc` and IDE auto-complete surface a `health`
//! namespace alongside `tenants`, `indexes`, etc.
