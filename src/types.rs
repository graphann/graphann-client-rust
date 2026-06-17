//! Shared request and response types serialised across the SDK.
//!
//! Types match the JSON shapes produced by the GraphANN HTTP server. Field
//! attributes follow the server's Go conventions — `snake_case` keys,
//! `serde(default)` for optional inbound fields, and `skip_serializing_if`
//! on optional outbound fields so we never send `null` where the server
//! treats `null` and "absent" differently.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Generic JSON value alias. Used wherever the server returns
/// caller-defined metadata.
pub type JsonValue = serde_json::Value;

/// Wire-level health status returned by `GET /health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    /// `"healthy"` when the server is up.
    pub status: String,
}

/// Wire-level readiness response returned by `GET /ready`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ready {
    /// `"ready"` or `"not ready"`.
    pub status: String,
    /// Reason text when `status != "ready"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Build/version banner. The HTTP server does NOT yet expose `/version`;
/// this struct exists so downstream code can plumb the SDK's compiled
/// version into observability stacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// SDK version (compile-time).
    pub sdk_version: String,
    /// Server-reported version string when discoverable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
}

// =====================================================================
// Tenants
// =====================================================================

/// A tenant — the top-level isolation boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    /// Stable tenant identifier (`t_<uuid>`).
    pub id: String,
    /// Human-friendly name.
    pub name: String,
    /// RFC3339 creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// RFC3339 update timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Best-effort count of indexes belonging to this tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_count: Option<u64>,
    /// Provider metadata as exposed by the server (LLM settings etc.,
    /// API keys masked).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// `POST /v1/tenants` request body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateTenantRequest {
    /// Optional deterministic id. When set the server does an idempotent
    /// create-or-fetch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Human-friendly name.
    pub name: String,
}

/// `GET /v1/tenants` response envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListTenantsResponse {
    /// Tenants on this page.
    #[serde(default)]
    pub tenants: Vec<Tenant>,
    /// Total tenants matched.
    #[serde(default)]
    pub total: u64,
}

// =====================================================================
// Indexes
// =====================================================================

/// Index lifecycle status as reported by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    /// Stable index id (`i_<uuid>`).
    pub id: String,
    /// Owning tenant id.
    pub tenant_id: String,
    /// Display name.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Lifecycle status: `empty`, `building`, `ready`, `error`.
    pub status: String,
    /// Number of documents currently indexed.
    #[serde(default)]
    pub num_docs: u64,
    /// Number of chunks currently indexed.
    #[serde(default)]
    pub num_chunks: u64,
    /// Embedding dimension; zero until the first document is embedded.
    #[serde(default)]
    pub dimension: u32,
    /// RFC3339 creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// RFC3339 update timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// Filesystem path under the data dir. Populated on org/user/shared
    /// listings; omitted for tenant-scoped routes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// User id of the creator. Populated on org-scoped listings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// Free-form metadata bag. Populated on org-scoped listings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// Compression strategy used by the index (e.g. `"pq"`, `"scalar"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
    /// Whether the index uses approximate (HNSW) search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
}

/// Status response from `GET /v1/tenants/.../indexes/.../status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    /// Index id.
    pub index_id: String,
    /// `empty`, `building`, `ready`, or `error`.
    pub status: String,
    /// Populated when `status == "error"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `POST /v1/tenants/.../indexes` body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateIndexRequest {
    /// Optional deterministic id (idempotent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Display name.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Compression strategy (`"none"`, `"scalar"`, `"binary"`, `"pq"`,
    /// `"recompute"`, or `""` for server default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
    /// When `true`, build an approximate (HNSW) graph; `false` uses brute force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
}

/// `PATCH /v1/tenants/.../indexes/.../` body. Only the supplied keys are
/// updated server-side (partial merge).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateIndexRequest {
    /// New display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Compression strategy to apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
    /// Toggle approximate (HNSW) search on/off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
}

/// Live (in-memory) index statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveIndexStats {
    /// Index id.
    pub index_id: String,
    /// `true` if currently loaded into memory.
    #[serde(default)]
    pub is_live: bool,
    /// Chunks in the compacted base layer.
    #[serde(default)]
    pub base_chunks: u64,
    /// Chunks in the delta layer (post last compaction).
    #[serde(default)]
    pub delta_chunks: u64,
    /// Total chunks (base + delta).
    #[serde(default)]
    pub total_chunks: u64,
    /// Tombstoned chunks (excluded from search).
    #[serde(default)]
    pub deleted_chunks: u64,
    /// Active chunks (`total - deleted`).
    #[serde(default)]
    pub live_chunks: u64,
    /// Document count.
    #[serde(default)]
    pub documents: u64,
    /// Embedding dimension, when known.
    #[serde(default)]
    pub dimension: u32,
    /// `true` when there are unsaved changes.
    #[serde(default)]
    pub is_dirty: bool,
    /// Set on the alternate "not live" response shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_chunks: Option<u64>,
    /// Set on the alternate "not live" response shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub num_docs: Option<u64>,
}

/// `GET /v1/tenants/.../indexes` envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListIndexesResponse {
    /// Indexes on this page.
    #[serde(default)]
    pub indexes: Vec<Index>,
    /// Total indexes matched.
    #[serde(default)]
    pub total: u64,
}

/// Response from `POST /v1/tenants/.../indexes/.../flush`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlushResponse {
    /// `true` when the flush completed.
    #[serde(default)]
    pub flushed: bool,
}

/// Response from `POST /v1/tenants/.../indexes/.../rebuild-graph`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RebuildGraphResponse {
    /// `true` when the delta graph was rebuilt.
    #[serde(default)]
    pub rebuilt: bool,
    /// Number of chunks re-inserted into the rebuilt graph.
    #[serde(default)]
    pub chunks: u64,
    /// Wall-clock rebuild time in milliseconds.
    #[serde(default)]
    pub wall_ms: u64,
}

// =====================================================================
// Documents
// =====================================================================

/// A document submitted via `add_documents` / `import_documents`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    /// Optional client-supplied id (also called external id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Document text. The server also accepts `content` as an alias.
    pub text: String,
    /// Optional structured metadata; round-tripped verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonValue>,
    /// When `true`, prior chunks for `id` are replaced atomically.
    #[serde(default, skip_serializing_if = "is_false")]
    pub upsert: bool,
    /// Optional RFC3339 expiry; chunks become invisible to search after
    /// this point and are eligible for GC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// RBAC: repository identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    /// RBAC: file path within the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// RBAC: source git commit sha.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    /// Precomputed embedding vector. When **every** document in the
    /// batch carries a non-empty vector the server skips embedding and
    /// ingests the supplied vectors directly. Mixed batches (some with,
    /// some without) are rejected with HTTP 400. Length must match the
    /// index dimension once it is fixed; a fresh index (dimension 0)
    /// accepts any length and the first ingest fixes the dimension.
    ///
    /// On the precomputed path inserts are idempotent by external id
    /// (server-side upsert), so the per-document [`Document::upsert`]
    /// delete-then-add pre-pass is intentionally not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// `POST .../documents` body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddDocumentsRequest {
    /// Documents to add.
    pub documents: Vec<Document>,
    /// Skip the per-batch full-delta save. Ingested data stays in
    /// memory (index dirty) but is still searchable; persist it later
    /// via [`crate::Client::flush_index`]. Default `false` preserves
    /// the per-batch, immediately-persisted behaviour.
    #[serde(default, skip_serializing_if = "is_false")]
    pub defer_save: bool,
    /// Bulk-load mode. Implies `defer_save` **and** defers the per-node
    /// HNSW insert — the delta graph is built once, concurrently, at
    /// [`crate::Client::flush_index`]. Bulk-ingested data is NOT
    /// searchable until the flush builds the graph, with one safety
    /// net: the first search against a pending deferred build
    /// transparently triggers it server-side (build-on-read), so
    /// searches never silently miss bulk data. Default `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub bulk: bool,
}

/// `POST .../documents` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddDocumentsResponse {
    /// Number accepted.
    #[serde(default)]
    pub added: u64,
    /// Index id (echoed).
    #[serde(default)]
    pub index_id: String,
    /// String (UUID) chunk ids assigned to the new chunks. Server emits
    /// `[]store.ChunkID` (= `[]string`); decoding as `Vec<i64>` fails.
    #[serde(default)]
    pub chunk_ids: Vec<String>,
    /// External ids, one per submitted document and positionally
    /// aligned with the request array. Present only when the server
    /// minted at least one id (sharded ingest of id-less documents —
    /// the external id is the shard routing key); when present it
    /// carries ALL ids, minted and client-supplied alike. Persist
    /// these as the durable document ids. `None` on unsharded
    /// deployments and older servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ids: Option<Vec<String>>,
}

/// `POST .../import` body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportDocumentsRequest {
    /// Documents to queue.
    pub documents: Vec<Document>,
}

/// `POST .../import` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportDocumentsResponse {
    /// Index id (echoed).
    #[serde(default)]
    pub index_id: String,
    /// Number of documents accepted onto the queue.
    #[serde(default)]
    pub imported: u64,
    /// Numeric document ids assigned.
    #[serde(default)]
    pub document_ids: Vec<i64>,
    /// Total queue depth after this call.
    #[serde(default)]
    pub pending_total: u64,
    /// e.g. `"processing"`.
    #[serde(default)]
    pub status: String,
    /// Optional human message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Pending queue snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PendingStatus {
    /// Index id.
    #[serde(default)]
    pub index_id: String,
    /// Documents waiting to be embedded / indexed.
    #[serde(default)]
    pub pending_count: u64,
}

/// Bulk delete by numeric document id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BulkDeleteDocumentsRequest {
    /// Document ids to remove.
    pub document_ids: Vec<i64>,
}

/// Response for either bulk delete variant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BulkDeleteResponse {
    /// Index id (echoed).
    #[serde(default)]
    pub index_id: String,
    /// Number of documents that matched.
    #[serde(default)]
    pub documents_deleted: u64,
    /// Number of chunks tombstoned.
    #[serde(default)]
    pub chunks_deleted: u64,
    /// Per-doc breakdown when keying by numeric id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_per_doc: Option<HashMap<String, u64>>,
    /// Per-id breakdown when keying by external id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_per_id: Option<HashMap<String, u64>>,
}

/// Bulk delete by client-supplied external id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BulkDeleteByExternalIdsRequest {
    /// External ids to remove.
    pub external_ids: Vec<String>,
}

/// Cleanup orphans response.
///
/// `min_age` is a Go-style duration string echoing the cutoff the server
/// actually applied (e.g. `"1h0m0s"`, `"24h0m0s"`). `dry_run` echoes the
/// dry-run flag — when `true`, `removed` is what would have been deleted,
/// not what was deleted. Both fields default to empty/false when missing
/// (older servers).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanupOrphansResponse {
    /// Paths removed (or that would have been removed in dry-run mode).
    #[serde(default)]
    pub removed: Vec<String>,
    /// Bytes reclaimed (or that would have been reclaimed in dry-run mode).
    #[serde(default)]
    pub freed_bytes: u64,
    /// Echo of the minimum-age cutoff the server applied.
    #[serde(default)]
    pub min_age: String,
    /// Echo of the dry-run flag.
    #[serde(default)]
    pub dry_run: bool,
}

/// Response body for `POST .../indexes/{id}/gc` and `POST /v1/admin/gc`.
/// Both endpoints share the same shape: the index id (empty when admin
/// scope) and the count of expired documents reclaimed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GCResponse {
    /// Index id, present on per-index GC, empty for admin GC.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub index_id: String,
    /// Number of expired documents removed.
    #[serde(default)]
    pub deleted_count: u64,
}

/// One row of the prefix-list response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentEntry {
    /// External id.
    pub id: String,
    /// Reconstructed text. Empty when unavailable.
    #[serde(default)]
    pub text: String,
    /// Structured metadata (when stored).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, JsonValue>>,
}

/// Page returned by `list_documents`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListDocumentsPage {
    /// Documents in this page.
    #[serde(default)]
    pub documents: Vec<DocumentEntry>,
    /// Cursor for the next page; absent when exhausted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Result of `delete_document` (single doc).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteDocumentResponse {
    /// Index id (echoed).
    #[serde(default)]
    pub index_id: String,
    /// Numeric document id.
    #[serde(default)]
    pub document_id: i64,
    /// Chunks tombstoned.
    #[serde(default)]
    pub deleted_chunks: u64,
}

// =====================================================================
// Chunks (per-chunk read/delete)
// =====================================================================

/// Body returned by `GET /v1/tenants/.../chunks/{chunkID}`.
///
/// The server returns a flat object so the SDK keeps the same shape
/// (no envelope) even though `WriteSuccess` adds a top-level wrapper
/// in some other endpoints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Chunk {
    /// Numeric chunk id.
    #[serde(default)]
    pub chunk_id: i64,
    /// Reconstructed text. May be empty if the chunk was tombstoned
    /// since the request started.
    #[serde(default)]
    pub text: String,
    /// Owning numeric document id; -1 when metadata could not be loaded.
    #[serde(default)]
    pub document_id: i64,
    /// Position of this chunk within its parent document (0-based).
    #[serde(default)]
    pub chunk_index: i64,
    /// Byte offset of the chunk inside the original document.
    #[serde(default)]
    pub start: i64,
    /// Byte offset of the chunk's end inside the original document.
    #[serde(default)]
    pub end: i64,
}

/// Body returned by `DELETE /v1/tenants/.../chunks/{chunkID}` for a
/// single chunk delete. The server tombstones one chunk at a time and
/// echoes the index id back.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteChunkResponse {
    /// Index id (echoed).
    #[serde(default)]
    pub index_id: String,
    /// Total chunks tombstoned by this call (typically 1).
    #[serde(default)]
    pub deleted: u64,
}

/// Body sent by `DELETE /v1/tenants/.../chunks/{chunkID}` for a batch
/// chunk delete. The server reads the `chunk_ids` list from the body
/// and ignores the trailing path segment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteChunksRequest {
    /// Numeric chunk ids to tombstone. Must be non-empty.
    #[serde(default)]
    pub chunk_ids: Vec<i64>,
}

/// Body returned by `DELETE /v1/tenants/.../chunks/{chunkID}` for a
/// batch chunk delete. Mirrors [`DeleteChunkResponse`] but reports the
/// total tombstoned across the supplied `chunk_ids`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeleteChunksResponse {
    /// Index id (echoed).
    #[serde(default)]
    pub index_id: String,
    /// Total chunks tombstoned by this call.
    #[serde(default)]
    pub deleted: u64,
}

// =====================================================================
// Search
// =====================================================================

/// `POST .../search` request body.
///
/// Supply either `query` (text; embedded server-side) or `vector`
/// (pre-computed embedding). Both fields may be set for a hybrid search.
///
/// The `rerank`/`candidate_k`/`rerank_k` fields opt in to cross-encoder
/// reranking when the server has a reranker configured (via
/// `--reranker-url`). Silently no-op against servers without one — safe
/// to set unconditionally.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchRequest {
    /// Text query — embedded server-side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Pre-computed embedding vector (must match the index dimension).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<Vec<f32>>,
    /// Number of results to return. The server clamps with a maximum.
    #[serde(default = "default_k")]
    pub k: u32,
    /// Optional filter for RBAC / metadata pruning.
    #[serde(default, skip_serializing_if = "SearchFilter::is_empty")]
    pub filter: SearchFilter,
    /// Enable cross-encoder rerank of the top-`candidate_k` HNSW
    /// candidates. Effective only when the server has a reranker
    /// configured AND `query` (text) is supplied — vector-only requests
    /// have no text to feed the cross-encoder. Defaults to `false`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rerank: bool,
    /// First-stage candidate pool size fed to the reranker. Effective
    /// only when `rerank` is true. `None` (the default) tells the
    /// server to use `max(4*k, 50)`. The server clamps to `[k, 1000]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_k: Option<u32>,
    /// Number of results to return AFTER reranking. Effective only when
    /// `rerank` is true. `None` defaults to `k`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_k: Option<u32>,
    /// Per-query HNSW search expansion factor. `None` (or `Some(0)`)
    /// uses the server default (`--search-ef`, 64 unless overridden).
    /// The server clamps rather than rejects: values are capped at
    /// 2000. Mode-local floors may raise the effective ef on top of
    /// the request (scalar-quant and guided-recompute paths), and
    /// binary/PQ flat scans ignore ef entirely. The identical clamp is
    /// applied on the cluster shard-query bridge, so sharded searches
    /// honour the same bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ef_search: Option<u32>,
}

fn default_k() -> u32 {
    10
}

/// Search filter — limits the result set to chunks matching every clause.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilter {
    /// Limit to chunks attributed to these repositories.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repo_ids: Vec<String>,
    /// Strip chunks with these external ids from the result set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_external_ids: Vec<String>,
    /// Require each key/value to match the chunk's stored metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata_filter: HashMap<String, JsonValue>,
    /// Generic equality pre-filter: every key must match the chunk's
    /// stored metadata exactly. Takes precedence over `metadata_filter`
    /// on the server when both are present.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub equals: HashMap<String, String>,
}

impl SearchFilter {
    /// Returns `true` when no filter clauses are set; used by serde to
    /// drop the field entirely instead of sending `{}`.
    pub fn is_empty(&self) -> bool {
        self.repo_ids.is_empty()
            && self.exclude_external_ids.is_empty()
            && self.metadata_filter.is_empty()
            && self.equals.is_empty()
    }
}

/// One result returned from `search`.
///
/// `score` is always the first-stage cosine similarity (higher is
/// better) regardless of whether reranking ran. `rerank_score` is
/// `Some(_)` only when the server actually applied the cross-encoder
/// reranker to this entry — it carries the reranker's native score
/// (different scale, typically roughly -10..10 for bge-reranker-v2-m3)
/// and drives the result ordering. When `None`, ordering is by
/// `score` and the response can be treated as a plain non-rerank
/// search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResult {
    /// Stable chunk identifier — string form used over the wire.
    #[serde(default)]
    pub id: String,
    /// Chunk text (when included).
    #[serde(default)]
    pub text: String,
    /// First-stage cosine similarity. Higher is better.
    #[serde(default)]
    pub score: f32,
    /// Cross-encoder relevance score, populated only when the server
    /// actually reranked this entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_score: Option<f32>,
    /// Optional structured metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonValue>,
}

/// Search response envelope.
///
/// The `partial` / `shards_total` / `shards_ok` / `degraded_shards`
/// fields are additive and appear **only** on the sharded
/// scatter-gather path (cluster deployments where the index spans more
/// than one shard). Single-node and single-shard deployments return
/// the pre-sharding `{results, total}` shape — treat every sharded
/// field as optional. Sharded caveats: `rerank` / `candidate_k` /
/// `rerank_k` are not applied on the sharded path, text queries are
/// embedded once on the coordinator, and results are deduplicated by
/// external id keeping the highest score.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Hits, ordered by relevance.
    #[serde(default)]
    pub results: Vec<SearchResult>,
    /// Total hits returned in `results`. The server caps with k.
    #[serde(default)]
    pub total: u64,
    /// `Some(true)` when at least one shard contributed nothing to the
    /// result set. Always present on the sharded path; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial: Option<bool>,
    /// Total shards the query fanned out to (sharded path only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shards_total: Option<u32>,
    /// Shards that answered successfully (sharded path only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shards_ok: Option<u32>,
    /// Ids of shards that degraded the response. Present only when
    /// non-empty, even on the sharded path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_shards: Option<Vec<String>>,
}

/// Org-level multi-source search request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultiSearchRequest {
    /// Search query (free text).
    pub query: String,
    /// Number of results to return.
    #[serde(default = "default_k")]
    pub k: u32,
    /// Restrict to specific source types (e.g. `github`, `confluence`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// Search expansion factor — higher trades recall for latency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ef_search: Option<u32>,
    /// Whether to include the chunk text in each result.
    #[serde(default, skip_serializing_if = "is_false")]
    pub include_text: bool,
    /// Restrict to documents created at or after this Unix timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time: Option<i64>,
    /// Restrict to documents created at or before this Unix timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    /// Drop hits whose distance to the query exceeds this threshold
    /// (lower distance = closer). `None` (the default) applies no cutoff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_threshold: Option<f32>,
}

/// Hit returned by org-level search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultiSearchResult {
    /// Chunk id.
    #[serde(default)]
    pub chunk_id: i64,
    /// Chunk text (only when `include_text` was set).
    #[serde(default)]
    pub text: String,
    /// Distance to query (lower is closer).
    #[serde(default)]
    pub distance: f32,
    /// Source type.
    #[serde(default)]
    pub source_type: String,
    /// Repository id (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    /// Unix timestamp when the source content was created.
    #[serde(default)]
    pub created_at: i64,
    /// `true` when the hit came from a shared index.
    #[serde(default)]
    pub shared: bool,
    /// Free-form metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonValue>,
}

/// Response envelope for multi-source search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultiSearchResponse {
    /// Hits, ordered by relevance.
    #[serde(default)]
    pub results: Vec<MultiSearchResult>,
    /// Total returned.
    #[serde(default)]
    pub total: u64,
    /// Echoed query string.
    #[serde(default)]
    pub query: String,
    /// Org id (echoed).
    #[serde(default)]
    pub org_id: String,
    /// User id (echoed).
    #[serde(default)]
    pub user_id: String,
}

// =====================================================================
// Jobs (hot model switch)
// =====================================================================

/// Embedding-model switch request body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SwitchEmbeddingModelRequest {
    /// Backend ("ollama", "openai", "local_onnx").
    pub embedding_backend: String,
    /// Model identifier.
    pub model: String,
    /// Embedding dimension produced by the model.
    pub dimension: u32,
    /// Optional endpoint override (URL or local path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_override: Option<String>,
    /// Optional API key — never logged or echoed back by the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// Job status string values — typed for compile-time safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Created, not yet picked up.
    Queued,
    /// Currently running.
    Running,
    /// Finished without error.
    Completed,
    /// Finished with an error (see `Job::error`).
    Failed,
}

/// Job kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobKind {
    /// Reembed (hot model switch).
    Reembed,
}

/// Progress snapshot included with each job poll.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobProgress {
    /// Chunks processed so far.
    #[serde(default)]
    pub chunks_done: u64,
    /// Total chunks expected.
    #[serde(default)]
    pub chunks_total: u64,
}

/// Job envelope returned by `get_job` and listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Job id (`job_<uuid>`).
    pub job_id: String,
    /// Job kind.
    pub kind: JobKind,
    /// Owning tenant id.
    pub tenant_id: String,
    /// Target index id.
    pub index_id: String,
    /// Lifecycle status.
    pub status: JobStatus,
    /// Progress snapshot.
    #[serde(default)]
    pub progress: JobProgress,
    /// Set when status moved to running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    /// Set when status moved to a terminal state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// Error message when status == failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// RFC3339 creation timestamp.
    pub created_at: String,
}

/// Response from `PATCH .../embedding-model`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchEmbeddingModelResponse {
    /// Newly created job id.
    pub job_id: String,
    /// Initial status (always `queued`).
    pub status: JobStatus,
}

/// `GET /v1/jobs` and tenant-scoped variant envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListJobsResponse {
    /// Jobs on this page.
    #[serde(default)]
    pub jobs: Vec<Job>,
    /// Total returned.
    #[serde(default)]
    pub total: u64,
    /// Cursor for the next page (absent when exhausted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Filter for listing jobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListJobsFilter {
    /// Filter by job status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<JobStatus>,
    /// Pagination cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// Page size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

// =====================================================================
// Cluster
// =====================================================================

/// Per-node entry in `GET /v1/cluster/nodes`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Node id.
    #[serde(default)]
    pub id: String,
    /// Listen address (`host:port`).
    #[serde(default)]
    pub addr: String,
    /// Failure-domain zone, when configured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// `"alive" | "suspect" | "dead"`.
    #[serde(default)]
    pub state: String,
    /// RFC3339 timestamp of the last heartbeat seen.
    #[serde(default)]
    pub last_seen: String,
}

/// Cluster shard placement entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterShard {
    /// Shard id.
    #[serde(default)]
    pub id: String,
    /// Node id of the primary replica.
    #[serde(default)]
    pub primary: String,
    /// Replica node ids (may include `primary`).
    #[serde(default)]
    pub replicas: Vec<String>,
    /// Optional zone -> node mapping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone_placement: Option<HashMap<String, String>>,
}

/// `GET /v1/cluster/nodes` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterNodesResponse {
    /// All known nodes.
    #[serde(default)]
    pub nodes: Vec<ClusterNode>,
    /// Current Raft leader id (empty when no leader).
    #[serde(default)]
    pub leader: String,
}

/// `GET /v1/cluster/shards` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterShardsResponse {
    /// All shards.
    #[serde(default)]
    pub shards: Vec<ClusterShard>,
    /// Replication factor.
    #[serde(default)]
    pub rf: u32,
}

/// `GET /v1/cluster/health` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterHealth {
    /// `"ok" | "degraded" | "unhealthy"`.
    #[serde(default)]
    pub status: String,
    /// Total members in the gossip group.
    #[serde(default)]
    pub cluster_size: u32,
    /// Members reporting `state == "alive"`.
    #[serde(default)]
    pub alive_nodes: u32,
    /// Whether Raft has an elected leader.
    #[serde(default)]
    pub raft_has_leader: bool,
    /// Number of shards below their replication factor.
    #[serde(default)]
    pub under_replicated_shards: u32,
}

// =====================================================================
// Resources (atomic upsert)
// =====================================================================

/// Body for `PUT /v1/tenants/.../indexes/.../resources/{resourceID}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpsertResourceRequest {
    /// Resource text — chunked and embedded server-side.
    pub text: String,
    /// Optional metadata attached to every chunk produced from this resource.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Response from `upsert_resource`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpsertResourceResponse {
    /// The resource id (echoed).
    #[serde(default)]
    pub resource_id: String,
    /// Chunks created in this call.
    #[serde(default)]
    pub chunks_added: u64,
    /// Prior chunks tombstoned for this resource (on update).
    #[serde(default)]
    pub chunks_tombstoned: u64,
    /// `"create"` (first call for this resource) or `"update"`.
    #[serde(default)]
    pub operation: String,
}

// =====================================================================
// LLM Settings (per org)
// =====================================================================

/// LLM configuration for an org / tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LlmSettings {
    /// Provider: `openai`, `ollama`, `anthropic`.
    #[serde(default)]
    pub provider: String,
    /// Model identifier.
    #[serde(default)]
    pub model: String,
    /// Optional API key — server returns this masked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Optional base URL override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Sampling temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum tokens per reply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

// =====================================================================
// API Keys
// =====================================================================

/// Per-key projection returned by the list endpoint — never carries the
/// plaintext value. Field set matches the server's `APIKeyListItem`
/// (`internal/server/apikey_handlers.go`): `{ id, user_id, name,
/// created_at, last_used_at }`. There is no `prefix`, `expires_at`, or
/// `description` on the wire.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiKey {
    /// Key id.
    #[serde(default)]
    pub id: String,
    /// Owning user id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Human-readable label for the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last-used timestamp, when the server tracks it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

/// Response from `create_api_key`. Includes the **plaintext key**, returned
/// only on the create response — store it client-side immediately. Shape
/// matches the server's `CreateAPIKeyResponse`: `{ id, name, user_id,
/// plaintext, created_at }`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    /// Key id.
    #[serde(default)]
    pub id: String,
    /// Human-readable label for the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Owning user id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// One-time plaintext value. Returned exactly once at create time;
    /// the server only stores its argon2id hash, so persist it now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plaintext: Option<String>,
    /// Creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Body for `create_api_key`. Both fields are sent; the server reads both
/// (`internal/server/apikey_handlers.go`). `user_id` may be empty.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    /// Owning user id within the tenant. Empty string is accepted.
    #[serde(default)]
    pub user_id: String,
    /// Human-readable label for the key.
    #[serde(default)]
    pub name: String,
}

/// Listing envelope returned by `list_api_keys`. The wrapper key is
/// `api_keys`, matching the server's `ListAPIKeysResponse`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListApiKeysResponse {
    /// Keys on this page.
    #[serde(default)]
    pub api_keys: Vec<ApiKey>,
}

// =====================================================================
// Org-level index listings
// =====================================================================

/// `GET /v1/orgs/{orgID}/users/{userID}/indexes` envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListUserIndexesResponse {
    /// Personal indexes belonging to the user.
    #[serde(default)]
    pub indexes: Vec<Index>,
    /// Total returned.
    #[serde(default)]
    pub total: u64,
    /// Org id (echoed).
    #[serde(default)]
    pub org_id: String,
    /// User id (echoed).
    #[serde(default)]
    pub user_id: String,
}

/// `GET /v1/orgs/{orgID}/shared/indexes` envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListSharedIndexesResponse {
    /// Shared indexes belonging to the org.
    #[serde(default)]
    pub indexes: Vec<Index>,
    /// Total returned.
    #[serde(default)]
    pub total: u64,
    /// Org id (echoed).
    #[serde(default)]
    pub org_id: String,
}

// =====================================================================
// Org-level sync (used by `multi_search` and friends)
// =====================================================================

/// Sync documents body for `POST /v1/orgs/{orgID}/documents`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncDocumentsRequest {
    /// Acting user.
    pub user_id: String,
    /// Free-form source type (`github`, `confluence`, ...).
    pub source_type: String,
    /// `true` for shared/dedup, `false` for per-user index.
    #[serde(default)]
    pub shared: bool,
    /// Documents to ingest.
    pub documents: Vec<SyncDocument>,
}

/// Document used by `SyncDocumentsRequest`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncDocument {
    /// Stable upstream id (required for shared, used for dedup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    /// Document text.
    pub text: String,
    /// Optional metadata map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
}

/// Response from `sync_documents`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncDocumentsResponse {
    /// Number of documents synced.
    #[serde(default)]
    pub synced: u64,
    /// Echoed org id.
    #[serde(default)]
    pub org_id: String,
    /// Echoed user id.
    #[serde(default)]
    pub user_id: String,
    /// Echoed source type.
    #[serde(default)]
    pub source_type: String,
    /// `"shared"` or `"personal"`.
    #[serde(default)]
    pub index_type: String,
}
