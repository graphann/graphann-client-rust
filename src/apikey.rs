//! API-key management endpoints on [`crate::Client`].
//!
//! Mirrors the server-side surface in `internal/server/routes.go`:
//! per-tenant CRUD with no per-key GET (server only exposes
//! POST/GET-list/DELETE).

use reqwest::Method;

use crate::client::Client;
use crate::error::Error;
use crate::types::{CreateApiKeyRequest, CreateApiKeyResponse, ListApiKeysResponse};

impl Client {
    /// `POST /v1/tenants/{tenantID}/api-keys` — provision a new API key.
    /// Plaintext value is returned exactly once on this response.
    pub async fn create_api_key(
        &self,
        req: CreateApiKeyRequest,
    ) -> Result<CreateApiKeyResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/api-keys", tenant);
        self.request_json(Method::POST, &path, Some(&req)).await
    }

    /// `GET /v1/tenants/{tenantID}/api-keys`.
    pub async fn list_api_keys(&self) -> Result<ListApiKeysResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/api-keys", tenant);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `DELETE /v1/tenants/{tenantID}/api-keys/{keyID}`.
    pub async fn revoke_api_key(&self, key_id: &str) -> Result<(), Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/api-keys/{}", tenant, key_id);
        self.request_empty(Method::DELETE, &path, Option::<&()>::None)
            .await
    }
}
