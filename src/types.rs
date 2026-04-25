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
}

/// `PATCH /v1/tenants/.../indexes/.../` body. Both fields are optional
/// — only the supplied keys are updated server-side.
///
/// The current server returns 501 for this route; the type is provided
/// so callers can opt in once the server lights it up.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateIndexRequest {
    /// New display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// New description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// `POST .../documents` body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddDocumentsRequest {
    /// Documents to add.
    pub documents: Vec<Document>,
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
    /// Numeric chunk ids assigned to the new chunks.
    #[serde(default)]
    pub chunk_ids: Vec<i64>,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanupOrphansResponse {
    /// Paths removed.
    #[serde(default)]
    pub removed: Vec<String>,
    /// Bytes reclaimed.
    #[serde(default)]
    pub freed_bytes: u64,
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
// Search
// =====================================================================

/// Combined `search` / `search_text` / `search_vector` request body.
///
/// Either `query` or `vector` must be supplied (mutually exclusive on
/// the dedicated text/vector endpoints).
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
}

impl SearchFilter {
    /// Returns `true` when no filter clauses are set; used by serde to
    /// drop the field entirely instead of sending `{}`.
    pub fn is_empty(&self) -> bool {
        self.repo_ids.is_empty()
            && self.exclude_external_ids.is_empty()
            && self.metadata_filter.is_empty()
    }
}

/// One result returned from `search` / `search_text` / `search_vector`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResult {
    /// Stable chunk identifier — string form used over the wire.
    #[serde(default)]
    pub id: String,
    /// Chunk text (when included).
    #[serde(default)]
    pub text: String,
    /// Distance / similarity score. Lower is closer for L2 / cosine
    /// distance modes; the server picks the metric per index.
    #[serde(default)]
    pub score: f32,
    /// Optional structured metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonValue>,
}

/// Search response envelope.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Hits, ordered by relevance.
    #[serde(default)]
    pub results: Vec<SearchResult>,
    /// Total hits returned in `results`. The server caps with k.
    #[serde(default)]
    pub total: u64,
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

/// Public projection of an API key — never carries the plaintext value.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiKey {
    /// Key id.
    #[serde(default)]
    pub id: String,
    /// Owning user id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Display description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Key prefix (safe to display, e.g. first 4 chars).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last-used timestamp, when the server tracks it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    /// Optional expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Response from `create_api_key`. Includes the **plaintext key**, returned
/// only on the create response — store it client-side immediately.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateApiKeyResponse {
    /// Key metadata.
    #[serde(flatten)]
    pub key: ApiKey,
    /// One-time plaintext value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plaintext_key: Option<String>,
}

/// Body for `create_api_key`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    /// Owning user id within the tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Free-text description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Listing envelope returned by `list_api_keys`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListApiKeysResponse {
    /// Keys on this page.
    #[serde(default)]
    pub api_keys: Vec<ApiKey>,
    /// Total returned.
    #[serde(default)]
    pub total: u64,
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
