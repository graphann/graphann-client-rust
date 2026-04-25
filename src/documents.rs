//! Document ingestion + deletion + listing methods on [`crate::Client`].

use reqwest::Method;
use serde_json::json;

use crate::client::Client;
use crate::error::Error;
use crate::pagination::{Page, PageStream};
use crate::types::{
    AddDocumentsRequest, AddDocumentsResponse, BulkDeleteByExternalIdsRequest,
    BulkDeleteDocumentsRequest, BulkDeleteResponse, Chunk, CleanupOrphansResponse,
    DeleteChunkResponse, DeleteDocumentResponse, DocumentEntry, ImportDocumentsRequest,
    ImportDocumentsResponse, ListDocumentsPage, PendingStatus,
};

impl Client {
    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/documents`.
    pub async fn add_documents(
        &self,
        index_id: &str,
        req: AddDocumentsRequest,
    ) -> Result<AddDocumentsResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/documents", tenant, index_id);
        self.request_json(Method::POST, &path, Some(&req)).await
    }

    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/import`. Returns
    /// immediately — processing happens server-side in the background.
    pub async fn import_documents(
        &self,
        index_id: &str,
        req: ImportDocumentsRequest,
    ) -> Result<ImportDocumentsResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/import", tenant, index_id);
        self.request_json(Method::POST, &path, Some(&req)).await
    }

    /// `GET /v1/tenants/{tenantID}/indexes/{indexID}/pending`.
    pub async fn pending_status(&self, index_id: &str) -> Result<PendingStatus, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/pending", tenant, index_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/process`.
    pub async fn process_pending(&self, index_id: &str) -> Result<serde_json::Value, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/process", tenant, index_id);
        self.request_json::<serde_json::Value, _>(Method::POST, &path, Some(&json!({})))
            .await
    }

    /// `DELETE /v1/tenants/{tenantID}/indexes/{indexID}/pending`.
    pub async fn clear_pending(&self, index_id: &str) -> Result<serde_json::Value, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/pending", tenant, index_id);
        self.request_json::<(), serde_json::Value>(Method::DELETE, &path, Option::<&()>::None)
            .await
    }

    /// `GET /v1/tenants/{tenantID}/indexes/{indexID}/documents/{docID}`.
    pub async fn get_document(
        &self,
        index_id: &str,
        doc_id: i64,
    ) -> Result<serde_json::Value, Error> {
        let tenant = self.require_tenant()?;
        let path = format!(
            "v1/tenants/{}/indexes/{}/documents/{}",
            tenant, index_id, doc_id
        );
        self.request_json::<(), serde_json::Value>(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `DELETE /v1/tenants/{tenantID}/indexes/{indexID}/documents/{docID}`.
    pub async fn delete_document(
        &self,
        index_id: &str,
        doc_id: i64,
    ) -> Result<DeleteDocumentResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!(
            "v1/tenants/{}/indexes/{}/documents/{}",
            tenant, index_id, doc_id
        );
        self.request_json(Method::DELETE, &path, Option::<&()>::None)
            .await
    }

    /// `DELETE /v1/tenants/{tenantID}/indexes/{indexID}/documents` —
    /// bulk delete by numeric document id.
    pub async fn bulk_delete_documents(
        &self,
        index_id: &str,
        ids: Vec<i64>,
    ) -> Result<BulkDeleteResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/documents", tenant, index_id);
        let body = BulkDeleteDocumentsRequest { document_ids: ids };
        self.request_json(Method::DELETE, &path, Some(&body)).await
    }

    /// `DELETE /v1/tenants/{tenantID}/indexes/{indexID}/documents/by-external-id`.
    pub async fn bulk_delete_by_external_ids(
        &self,
        index_id: &str,
        ids: Vec<String>,
    ) -> Result<BulkDeleteResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!(
            "v1/tenants/{}/indexes/{}/documents/by-external-id",
            tenant, index_id
        );
        let body = BulkDeleteByExternalIdsRequest { external_ids: ids };
        self.request_json(Method::DELETE, &path, Some(&body)).await
    }

    /// `GET /v1/tenants/{tenantID}/indexes/{indexID}/chunks/{chunkID}` —
    /// fetch a single chunk by its numeric id.
    ///
    /// Returns [`crate::Error::NotFound`] when the chunk has been
    /// tombstoned or never existed.
    pub async fn get_chunk(&self, index_id: &str, chunk_id: i64) -> Result<Chunk, Error> {
        let tenant = self.require_tenant()?;
        let path = format!(
            "v1/tenants/{}/indexes/{}/chunks/{}",
            tenant, index_id, chunk_id
        );
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `DELETE /v1/tenants/{tenantID}/indexes/{indexID}/chunks/{chunkID}`
    /// — tombstone a single chunk. The server has no batch-delete chunk
    /// endpoint; delete the whole document via [`Client::delete_document`]
    /// when you need to drop every chunk for one doc.
    pub async fn delete_chunk(
        &self,
        index_id: &str,
        chunk_id: i64,
    ) -> Result<DeleteChunkResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!(
            "v1/tenants/{}/indexes/{}/chunks/{}",
            tenant, index_id, chunk_id
        );
        self.request_json(Method::DELETE, &path, Option::<&()>::None)
            .await
    }

    /// Stream of pages from `GET /v1/tenants/.../indexes/.../documents`.
    ///
    /// Use [`futures::TryStreamExt::try_next`] (or `tokio_stream::StreamExt`) to
    /// drive the stream. Each page contains its items plus a cursor for the
    /// following page; the stream completes once `next_cursor` is absent.
    pub fn list_documents(&self, index_id: &str) -> PageStream<DocumentEntry> {
        self.list_documents_with_prefix(index_id, "")
    }

    /// Same as [`Client::list_documents`] but filters by external-id prefix.
    pub fn list_documents_with_prefix(
        &self,
        index_id: &str,
        prefix: &str,
    ) -> PageStream<DocumentEntry> {
        let client = self.clone();
        let index_id = index_id.to_string();
        let prefix = prefix.to_string();

        PageStream::new(move |cursor: Option<String>| {
            let client = client.clone();
            let index_id = index_id.clone();
            let prefix = prefix.clone();
            async move {
                let tenant =
                    client.config().tenant_id.clone().ok_or_else(|| {
                        Error::Builder("no tenant id configured on client".into())
                    })?;
                let mut path = format!("v1/tenants/{}/indexes/{}/documents", tenant, index_id);
                let mut sep = '?';
                if !prefix.is_empty() {
                    path.push_str(&format!("{sep}prefix={}", urlencode(&prefix)));
                    sep = '&';
                }
                if let Some(c) = cursor.as_ref() {
                    path.push_str(&format!("{sep}cursor={}", urlencode(c)));
                }
                let resp: ListDocumentsPage = client
                    .request_json(Method::GET, &path, Option::<&()>::None)
                    .await?;
                Ok(Page {
                    items: resp.documents,
                    next_cursor: resp.next_cursor,
                })
            }
        })
    }

    /// `POST /v1/admin/cleanup-orphans` — admin-only.
    pub async fn cleanup_orphans(&self) -> Result<CleanupOrphansResponse, Error> {
        self.request_json(Method::POST, "v1/admin/cleanup-orphans", Some(&json!({})))
            .await
    }
}

/// Minimal in-tree URL encoder so we don't pull in `urlencoding`.
/// Encodes RFC3986 reserved sub-delims + `%` + space; leaves the
/// characters ASCII-alphanumeric / `-_.~` alone.
fn urlencode(input: &str) -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => {
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 0xf) as usize] as char);
            }
        }
    }
    out
}
