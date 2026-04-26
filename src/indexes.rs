//! Index-management methods on [`crate::Client`].
//!
//! Index ids are tenant-scoped. The methods here pick up the tenant from
//! the [`crate::ClientBuilder`]; alternative tenants must be supplied via
//! the explicit `*_in_tenant` variants.

use reqwest::Method;
use serde_json::json;

use crate::client::Client;
use crate::error::Error;
use crate::types::{
    CreateIndexRequest, Index, IndexStatus, ListIndexesResponse, LiveIndexStats, UpdateIndexRequest,
};

impl Client {
    /// `GET /v1/tenants/{tenantID}/indexes` (uses default tenant).
    pub async fn list_indexes(&self) -> Result<ListIndexesResponse, Error> {
        let tenant = self.require_tenant()?;
        self.list_indexes_in_tenant(tenant).await
    }

    /// `GET /v1/tenants/{tenantID}/indexes` for an explicit tenant id.
    pub async fn list_indexes_in_tenant(
        &self,
        tenant_id: &str,
    ) -> Result<ListIndexesResponse, Error> {
        let path = format!("v1/tenants/{}/indexes", tenant_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `POST /v1/tenants/{tenantID}/indexes`.
    pub async fn create_index(&self, req: CreateIndexRequest) -> Result<Index, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes", tenant);
        self.request_json(Method::POST, &path, Some(&req)).await
    }

    /// `GET /v1/tenants/{tenantID}/indexes/{indexID}`.
    pub async fn get_index(&self, index_id: &str) -> Result<Index, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}", tenant, index_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `DELETE /v1/tenants/{tenantID}/indexes/{indexID}`.
    pub async fn delete_index(&self, index_id: &str) -> Result<(), Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}", tenant, index_id);
        self.request_empty(Method::DELETE, &path, Option::<&()>::None)
            .await
    }

    /// `PATCH /v1/tenants/{tenantID}/indexes/{indexID}`.
    ///
    /// **Note:** the current GraphANN server returns
    /// `Error::Server { status: 501, .. }` (Not Implemented) — the route
    /// is wired but the underlying mutation isn't persisted yet.
    pub async fn update_index(
        &self,
        index_id: &str,
        req: UpdateIndexRequest,
    ) -> Result<Index, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}", tenant, index_id);
        self.request_json(Method::PATCH, &path, Some(&req)).await
    }

    /// `GET /v1/tenants/{tenantID}/indexes/{indexID}/status`.
    pub async fn get_index_status(&self, index_id: &str) -> Result<IndexStatus, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/status", tenant, index_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `GET /v1/tenants/{tenantID}/indexes/{indexID}/live-stats`.
    pub async fn get_live_stats(&self, index_id: &str) -> Result<LiveIndexStats, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/live-stats", tenant, index_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/clear`.
    pub async fn clear_index(&self, index_id: &str) -> Result<serde_json::Value, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/clear", tenant, index_id);
        self.request_json::<serde_json::Value, _>(Method::POST, &path, Some(&json!({})))
            .await
    }

    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/build` (deprecated).
    pub async fn build_index(&self, index_id: &str) -> Result<serde_json::Value, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/build", tenant, index_id);
        self.request_json::<serde_json::Value, _>(Method::POST, &path, Some(&json!({})))
            .await
    }

    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/compact`.
    pub async fn compact_index(&self, index_id: &str) -> Result<serde_json::Value, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/compact", tenant, index_id);
        self.request_json::<serde_json::Value, _>(Method::POST, &path, Some(&json!({})))
            .await
    }
}
