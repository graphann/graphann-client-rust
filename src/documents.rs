//! Document ingestion + deletion + listing methods on [`crate::Client`].

use std::time::Duration;

use reqwest::Method;
use serde_json::json;

use crate::client::Client;
use crate::error::Error;
use crate::pagination::{Page, PageStream};
use crate::types::{
    AddDocumentsRequest, AddDocumentsResponse, BulkDeleteByExternalIdsRequest,
    BulkDeleteDocumentsRequest, BulkDeleteResponse, Chunk, CleanupOrphansResponse,
    DeleteChunksRequest, DeleteChunksResponse, DeleteDocumentResponse, DocumentEntry, GCResponse,
    ImportDocumentsRequest, ImportDocumentsResponse, ListDocumentsPage, PendingStatus,
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
    pub async fn get_pending_status(&self, index_id: &str) -> Result<PendingStatus, Error> {
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
    /// — tombstone a batch of chunks in one round-trip.
    ///
    /// The server route nominally targets a single `{chunkID}` path
    /// segment but the handler reads `{"chunk_ids": [...]}` from the
    /// request body and ignores the path id. We send `0` as a placeholder
    /// to match the Go and Python SDKs' `DeleteChunks` semantics.
    pub async fn delete_chunks(
        &self,
        index_id: &str,
        chunk_ids: Vec<i64>,
    ) -> Result<DeleteChunksResponse, Error> {
        let tenant = self.require_tenant()?;
        // Path id is a sentinel — the body is the source of truth.
        let path = format!("v1/tenants/{}/indexes/{}/chunks/0", tenant, index_id);
        let body = DeleteChunksRequest { chunk_ids };
        self.request_json(Method::DELETE, &path, Some(&body)).await
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
    ///
    /// Sweeps stale compaction artifacts (`*.old` / `*.compact` / `*.backup`
    /// / `*.failed`) and pre-reembed snapshots (`*.pre-reembed.<timestamp>`)
    /// from every tenant's data tree.
    ///
    /// `min_age` is the minimum age before an artifact is eligible for
    /// removal. Pass [`Duration::ZERO`] to use the server default (1h). The
    /// server enforces a 5-minute floor — passing a smaller positive value
    /// is rejected with HTTP 400.
    ///
    /// When `dry_run` is `true`, the server enumerates what *would* have
    /// been removed without touching disk. The returned response echoes
    /// the effective `min_age` and `dry_run` so callers can confirm what
    /// happened.
    pub async fn cleanup_orphans(
        &self,
        min_age: Duration,
        dry_run: bool,
    ) -> Result<CleanupOrphansResponse, Error> {
        let mut path = String::from("v1/admin/cleanup-orphans");
        let mut sep = '?';
        if !min_age.is_zero() {
            path.push_str(&format!("{sep}min_age={}", format_duration(min_age)));
            sep = '&';
        }
        if dry_run {
            path.push_str(&format!("{sep}dry_run=true"));
        }
        self.request_json(Method::POST, &path, Some(&json!({})))
            .await
    }

    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/gc` — sweeps every
    /// document whose sidecar `expires_at` has passed and returns the count
    /// reclaimed. Idempotent — calling twice in a row returns 0 the second
    /// time.
    pub async fn run_index_gc(&self, index_id: &str) -> Result<GCResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/gc", tenant, index_id);
        self.request_json(Method::POST, &path, Some(&json!({})))
            .await
    }

    /// `POST /v1/admin/gc` — sweep expired documents across every loaded
    /// index in one shot. Admin-only.
    pub async fn run_admin_gc(&self) -> Result<GCResponse, Error> {
        self.request_json(Method::POST, "v1/admin/gc", Some(&json!({})))
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

/// Formats a [`Duration`] as a Go-style duration string the GraphANN
/// server understands (e.g. `"1h"`, `"24h0m0s"`, `"30m"`, `"500ms"`).
///
/// Mirrors Go's `time.Duration.String` output for the unit boundaries the
/// server cares about. Sub-second precision is emitted as plain
/// `"<n>ms"` / `"<n>µs"` / `"<n>ns"` to keep parsing trivial server-side.
fn format_duration(d: Duration) -> String {
    let total_nanos = d.as_nanos();
    if total_nanos == 0 {
        return "0s".to_string();
    }
    if total_nanos % 1_000 != 0 {
        // sub-microsecond precision; drop straight to nanoseconds.
        return format!("{}ns", total_nanos);
    }
    let total_micros = total_nanos / 1_000;
    if total_micros % 1_000 != 0 {
        return format!("{}µs", total_micros);
    }
    let total_millis = d.as_millis();
    if total_millis % 1_000 != 0 {
        return format!("{}ms", total_millis);
    }
    let total_secs = d.as_secs();
    let h = total_secs / 3_600;
    let m = (total_secs % 3_600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{}h{}m{}s", h, m, s)
    } else if m > 0 {
        format!("{}m{}s", m, s)
    } else {
        format!("{}s", s)
    }
}

#[cfg(test)]
mod duration_format_tests {
    use super::format_duration;
    use std::time::Duration;

    #[test]
    fn one_hour() {
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h0m0s");
    }

    #[test]
    fn twenty_four_hours() {
        assert_eq!(format_duration(Duration::from_secs(24 * 3600)), "24h0m0s");
    }

    #[test]
    fn thirty_minutes() {
        assert_eq!(format_duration(Duration::from_secs(30 * 60)), "30m0s");
    }

    #[test]
    fn five_minutes() {
        assert_eq!(format_duration(Duration::from_secs(5 * 60)), "5m0s");
    }

    #[test]
    fn five_hundred_ms() {
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
    }

    #[test]
    fn zero() {
        assert_eq!(format_duration(Duration::ZERO), "0s");
    }
}
