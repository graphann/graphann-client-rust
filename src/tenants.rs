//! Tenant CRUD methods on [`crate::Client`].

use reqwest::Method;

use crate::client::Client;
use crate::error::Error;
use crate::types::{CreateTenantRequest, ListTenantsResponse, Tenant};

impl Client {
    /// `GET /v1/tenants`. Lists every tenant the caller can see.
    pub async fn list_tenants(&self) -> Result<ListTenantsResponse, Error> {
        self.request_json(Method::GET, "v1/tenants", Option::<&()>::None)
            .await
    }

    /// `POST /v1/tenants`. Creates a new tenant. Pass an `id` on the
    /// request to make the call idempotent.
    pub async fn create_tenant(&self, req: CreateTenantRequest) -> Result<Tenant, Error> {
        self.request_json(Method::POST, "v1/tenants", Some(&req))
            .await
    }

    /// `GET /v1/tenants/{tenantID}`.
    pub async fn get_tenant(&self, tenant_id: &str) -> Result<Tenant, Error> {
        let path = format!("v1/tenants/{}", tenant_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `DELETE /v1/tenants/{tenantID}`. Removes the tenant and all its
    /// indexes. Idempotent at the route level — returns `Error::NotFound`
    /// if the tenant does not exist.
    pub async fn delete_tenant(&self, tenant_id: &str) -> Result<(), Error> {
        let path = format!("v1/tenants/{}", tenant_id);
        self.request_empty(Method::DELETE, &path, Option::<&()>::None)
            .await
    }
}
