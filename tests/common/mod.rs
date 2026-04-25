//! Shared wiremock fixtures used by the integration tests.

use std::time::Duration;

use graphann::{Client, ClientBuilder};
use wiremock::MockServer;

/// Build a mock-server-backed [`Client`] preconfigured with a tenant id and
/// API key so handler-tests can compare exact headers.
pub async fn fixture() -> (MockServer, Client) {
    let server = MockServer::start().await;
    let client = ClientBuilder::new()
        .base_url(server.uri())
        .unwrap()
        .api_key("t_test", "ak_test")
        .timeout(Duration::from_secs(5))
        .max_retries(2)
        .retry_base(Duration::from_millis(1))
        .retry_cap(Duration::from_millis(2))
        .build()
        .unwrap();
    (server, client)
}
