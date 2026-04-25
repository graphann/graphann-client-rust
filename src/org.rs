//! Org-scoped methods on [`crate::Client`].
//!
//! These routes live under `/v1/orgs/{orgID}/...` and are wired by the
//! server when the operator installs `OrgHandlers` (see
//! `internal/server/routes.go::setupOrgRoutes`). Self-hosted clusters
//! that ship without org handlers will return 404 here.

use reqwest::Method;

use crate::client::Client;
use crate::error::Error;
use crate::types::{
    ListSharedIndexesResponse, ListUserIndexesResponse, SyncDocumentsRequest,
    SyncDocumentsResponse,
};

impl Client {
    /// `POST /v1/orgs/{orgID}/documents` — unified document ingestion.
    ///
    /// `req.shared` controls which backing index receives the documents:
    /// - `true` routes to the org's shared dedup index
    ///   (`org/{orgID}/shared/{source_type}`).
    /// - `false` routes to the user's personal index
    ///   (`org/{orgID}/users/{user_id}/{source_type}`).
    ///
    /// Shared documents must carry a `resource_id` for deduplication;
    /// the server rejects the request otherwise.
    pub async fn sync_documents(
        &self,
        org_id: &str,
        req: SyncDocumentsRequest,
    ) -> Result<SyncDocumentsResponse, Error> {
        let path = format!("v1/orgs/{}/documents", org_id);
        self.request_json(Method::POST, &path, Some(&req)).await
    }

    /// `GET /v1/orgs/{orgID}/users/{userID}/indexes` — list a user's
    /// personal indexes within the given org.
    pub async fn list_user_indexes(
        &self,
        org_id: &str,
        user_id: &str,
    ) -> Result<ListUserIndexesResponse, Error> {
        let path = format!("v1/orgs/{}/users/{}/indexes", org_id, user_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `GET /v1/orgs/{orgID}/shared/indexes` — list every shared index
    /// the org owns (one per registered source type).
    pub async fn list_shared_indexes(
        &self,
        org_id: &str,
    ) -> Result<ListSharedIndexesResponse, Error> {
        let path = format!("v1/orgs/{}/shared/indexes", org_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }
}
