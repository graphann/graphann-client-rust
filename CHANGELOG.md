# Changelog

All notable changes to the `graphann` Rust SDK are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project adheres to [Semantic Versioning](https://semver.org/).

## [0.7.0] - 2026-06-10

### Added

- **Precomputed-vector ingest**: `Document::vector: Option<Vec<f32>>`.
  When every document in an `add_documents` batch carries a non-empty
  vector, the server skips embedding and ingests the vectors directly.
  Mixed batches (some with, some without) are rejected with HTTP 400.
  Vector length must match the index dimension once fixed; a fresh
  index (dimension 0) accepts any length and the first ingest fixes it.
  Precomputed inserts are idempotent by external id server-side, so the
  per-document `upsert` pre-delete pass does not run on this path. The
  16 MB request-body cap applies (≈1700 docs per precomputed batch).
- **Bulk-load ingest options**: `AddDocumentsRequest::defer_save` and
  `AddDocumentsRequest::bulk` (both default `false` — wire shape for
  default requests is unchanged). `defer_save` skips the per-batch
  full-delta save; data stays in memory (still searchable) until a
  flush. `bulk` implies `defer_save` and additionally defers the
  per-node HNSW insert — the delta graph is built once, concurrently,
  at flush. Bulk-ingested data is not searchable until then, except
  that the first search against a pending deferred build transparently
  triggers it server-side (build-on-read).
- `Client::flush_index(index_id)` / `BlockingClient::flush_index` —
  `POST .../flush`. Persists the live index's in-memory delta and
  builds any pending bulk-deferred graph in the same call. Sends `{}`
  with `Content-Type: application/json` (required by the server's
  middleware on every POST). Returns the new `FlushResponse`
  (`flushed: bool`).
- `Client::rebuild_graph(index_id)` / `BlockingClient::rebuild_graph`
  — `POST .../rebuild-graph`. In-place delta-HNSW rebuild; migration
  endpoint for indexes ingested before the 2026-06 neighbor-selection
  fix (fragmented delta graphs / reduced recall). Returns the new
  `RebuildGraphResponse` (`rebuilt`, `chunks`, `wall_ms`); 409 while a
  compaction is in flight maps to `Error::Conflict` — retry after a
  back-off.
- **Per-query search expansion factor**:
  `SearchRequest::ef_search: Option<u32>`. `None` (or `Some(0)`) uses
  the server default (`--search-ef`, 64 unless overridden). The server
  clamps rather than rejects (cap 2000); the identical clamp applies on
  the cluster shard-query bridge. Binary/PQ flat scans ignore ef;
  scalar-quant and guided-recompute paths may raise the effective ef
  via mode-local floors.
- **Sharded-search response fields** on `SearchResponse` (all optional;
  present only on the cluster scatter-gather path when an index spans
  more than one shard): `partial: Option<bool>`,
  `shards_total: Option<u32>`, `shards_ok: Option<u32>`, and
  `degraded_shards: Option<Vec<String>>` (present only when
  non-empty). Single-node deployments keep the byte-identical
  `{results, total}` shape. Sharded caveats: `rerank` / `candidate_k`
  / `rerank_k` are not applied on the sharded path; results are
  deduplicated by external id keeping the highest score.
- `AddDocumentsResponse::external_ids: Option<Vec<String>>` — one
  entry per submitted document, positionally aligned. Present only
  when the server minted at least one external id (sharded ingest of
  id-less documents); carries all ids (minted + client-supplied) when
  present. `None` on unsharded deployments and older servers.

### Changed

- `AddDocumentsRequest` gained fields; exact struct-literal
  construction (`AddDocumentsRequest { documents }`) needs
  `..Default::default()`, same as the `SearchRequest` field additions
  in 0.6.0. Wire behaviour of existing requests is unchanged — the new
  fields are skipped during serialisation at their defaults, so
  pre-0.7 servers (which reject unknown JSON fields) are unaffected.

### Unchanged

- Request-body gzip stays **opt-in** (`ClientBuilder::compress_requests`,
  default `false`) per 0.5.1 — no regression.
- `update_index` already supported `compression` / `approximate`
  (since 0.3.0); `compact_index` already sends `{}` with
  `Content-Type: application/json` and maps 409 to `Error::Conflict`;
  `cleanup_orphans` already takes `min_age` / `dry_run` (since 0.5.0).

## [0.6.0] - 2026-05-01

### Added

- `SearchRequest::rerank`, `SearchRequest::candidate_k`, and
  `SearchRequest::rerank_k` fields wire the optional cross-encoder
  reranker. When the server has a reranker configured (via
  `--reranker-url`), set `rerank: true` to rescore the top-`candidate_k`
  HNSW candidates with the reranker and return the top-`rerank_k` (or
  top-`k`). Defaults: `candidate_k = max(4*k, 50)` (server clamps to
  `[k, 1000]`), `rerank_k = k`. No-op against non-rerank-aware servers
  — safe to roll out unconditionally.
- `SearchResult::rerank_score: Option<f32>` — `Some(_)` only when the
  server actually reranked this entry. Carries the cross-encoder's
  native relevance score (different scale from cosine, typically
  -10..10 for bge-reranker-v2-m3) and reflects the result ordering.
  `None` means: server has no reranker, the request didn't ask for
  rerank, or the reranker errored and the server fell back to
  first-stage results.

### Unchanged

- `SearchResult::score` is still always the first-stage cosine
  similarity, regardless of rerank state. Existing client code that
  only reads `score` keeps working — even when accidentally hitting
  a rerank-enabled endpoint.

## [0.5.1] - 2026-04-30

### Fixed

- Request-body gzip is now opt-in. Versions ≤0.5.0 transparently
  gzipped any request body ≥64 KiB and emitted `Content-Encoding: gzip`,
  but the graphann HTTP server does not decode gzipped request bodies.
  The result was silent 400 "Invalid JSON body" errors on `add_documents`
  / `import_documents` batches that crossed the threshold (a single 50-doc
  geo-intel batch was enough to trigger it), surfaced by callers as
  intermittent failure modes that depended on payload size. Default
  builders now skip gzip regardless of body size; opt back in via
  `ClientBuilder::compress_requests(true)` when targeting an environment
  that decodes gzip (e.g. behind a proxy that decompresses before
  forwarding to graphann).

### Added

- `ClientBuilder::compress_requests(bool)` to opt into the previous
  auto-gzip behaviour when needed.

## [0.5.0] - 2026-04-30

### Changed (BREAKING)

- `Client::cleanup_orphans` / `BlockingClient::cleanup_orphans` now take
  `min_age: Duration` and `dry_run: bool` parameters. Pass
  `Duration::ZERO, false` to preserve the previous behaviour (server
  defaults: 1h min-age, real sweep). Server enforces a 5-minute floor on
  positive `min_age` values.
- `CleanupOrphansResponse` gains `min_age: String` (Go duration string
  echoed by the server) and `dry_run: bool` fields. Both default to
  empty/false on older servers.

## [0.3.0] - 2026-04-28

### Removed (BREAKING)

- `Client::search_text` / `BlockingClient::search_text` — endpoint deleted
  server-side. Use `Client::search` with `SearchRequest { query: Some(...),
  ..Default::default() }` instead.
- `Client::search_vector` / `BlockingClient::search_vector` — endpoint deleted
  server-side. Use `Client::search` with `SearchRequest { vector: Some(...),
  ..Default::default() }` instead.
- `Client::build_index` / `BlockingClient::build_index` — was a no-op stub;
  endpoint removed server-side.

### Added

- `Client::upsert_resource(index_id, resource_id, req)` /
  `BlockingClient::upsert_resource` — `PUT
  .../resources/{resourceID}`. Atomic create-or-replace: chunks, embeds, and
  swaps prior resource chunks in one request. Returns `UpsertResourceResponse`
  with `resource_id`, `chunks_added`, `chunks_tombstoned`, `operation`
  (`"create"` | `"update"`).
- New types: `UpsertResourceRequest`, `UpsertResourceResponse`.

### Changed

- `CreateIndexRequest` and `UpdateIndexRequest` gain optional
  `compression: Option<String>` and `approximate: Option<bool>`.
- `Index` gains optional `compression: Option<String>` and
  `approximate: Option<bool>`.
- `SearchFilter` gains `equals: HashMap<String, String>` for generic metadata
  pre-filtering. `SearchFilter::is_empty` updated accordingly.
- `compact_index` docstring documents that a 409 response maps to
  `Error::Conflict` (compaction already running — retry after back-off).
- `update_index` docstring drops the outdated "returns 501" note.

## [0.2.0] - 2026-04-25

### Changed (BREAKING)
- Method names on `Client` and `BlockingClient` standardized with the
  Go, Python, and TypeScript SDKs. Wire protocol unchanged — this is a
  source-only break, no behavior change. Migration table:

  | Old (`0.1.x`)        | New (`0.2.0`)            |
  | -------------------- | ------------------------ |
  | `pending_status`     | `get_pending_status`     |
  | `cluster_health`     | `get_cluster_health`     |
  | `cluster_nodes`      | `get_cluster_nodes`      |
  | `cluster_shards`     | `get_cluster_shards`     |
  | `live_index_stats`   | `get_live_stats`         |

- `delete_chunk(index_id, chunk_id: i64) -> DeleteChunkResponse` is
  replaced by `delete_chunks(index_id, chunk_ids: Vec<i64>) ->
  DeleteChunksResponse`. The server route is still
  `DELETE /v1/tenants/{tenantID}/indexes/{indexID}/chunks/{chunkID}`,
  but the handler reads `{"chunk_ids": [...]}` from the body and
  ignores the path id; the SDK now sends a single batched request with
  a sentinel `0` in the path, matching the Go and Python SDKs'
  `DeleteChunks` semantics. To migrate `client.delete_chunk(idx, id)`,
  use `client.delete_chunks(idx, vec![id])`.

### Added
- `DeleteChunksRequest` and `DeleteChunksResponse` in `types`. The
  former wraps `chunk_ids: Vec<i64>`; the latter mirrors the existing
  `DeleteChunkResponse` shape (`{deleted, index_id}`).

## [0.1.1] - 2026-04-25

### Added
- `Client::get_chunk(index_id, chunk_id)` and `Client::delete_chunk(index_id,
  chunk_id)` — per-chunk read + tombstone via
  `GET/DELETE /v1/tenants/{tenantID}/indexes/{indexID}/chunks/{chunkID}`.
  No batch chunk-delete endpoint exists server-side; loop on the per-chunk
  call when you need to drop several at once.
- `Client::sync_documents(org_id, req)` — unified org-scoped ingestion via
  `POST /v1/orgs/{orgID}/documents`. Routes to a shared dedup index when
  `req.shared` is true, otherwise to the user's personal index.
- `Client::list_user_indexes(org_id, user_id)` and
  `Client::list_shared_indexes(org_id)` — org-scoped index discovery via
  `GET /v1/orgs/{orgID}/users/{userID}/indexes` and
  `GET /v1/orgs/{orgID}/shared/indexes`.
- New types: `Chunk`, `DeleteChunkResponse`, `ListUserIndexesResponse`,
  `ListSharedIndexesResponse`. The `Index` struct gained optional `path`,
  `created_by`, and `metadata` fields populated by the org-scoped
  listings (omitted on tenant-scoped routes — backward compatible with
  0.1.0 deserialization).
- New `org` module groups the org-scoped methods alongside the existing
  per-domain modules.
- Blocking parity: `BlockingClient::{get_chunk, delete_chunk,
  sync_documents, list_user_indexes, list_shared_indexes}`.

### Changed
- **LLM settings path + method (server-side migration in lockstep):**
  `get_llm_settings`, `update_llm_settings`, and `delete_llm_settings`
  now hit `/v1/orgs/{orgID}/llm-settings` (was
  `/v1/orgs/{orgID}/settings/llm`, never wired in the default router).
  `update_llm_settings` is `PATCH` (partial merge) — was `PUT`.
- `update_llm_settings` and `delete_llm_settings` return `LlmSettings`
  (was `serde_json::Value`). PATCH responses carry the merged + masked
  settings; DELETE responses carry the package defaults.

### Removed (BREAKING)
- `Client::get_api_key(key_id)` / `BlockingClient::get_api_key(key_id)`.
  The route `GET /v1/tenants/{tenantID}/api-keys/{keyID}` is **not**
  registered server-side (see `internal/server/routes.go`); the method
  always returned `Error::NotFound`. Use `Client::list_api_keys` and
  filter client-side when you need a single key's metadata.

## [0.1.0] - 2026-04-25

### Added
- First public release of the Rust SDK.
- Async `Client` + `ClientBuilder` over `reqwest`.
- Optional sync `BlockingClient` behind the `blocking` feature.
- Cargo features: `rustls` (default), `native-tls`, `blocking`, `metrics`.
- Methods covering the GraphANN HTTP API:
  - Health: `health`, `ready`, `version`
  - Tenants: `list_tenants`, `create_tenant`, `get_tenant`, `delete_tenant`
  - Indexes: `list_indexes`, `create_index`, `get_index`, `delete_index`,
    `update_index`, `clear_index`, `build_index`, `compact_index`,
    `live_index_stats`, `get_index_status`
  - Documents: `add_documents`, `import_documents`, `pending_status`,
    `process_pending`, `clear_pending`, `get_document`, `delete_document`,
    `bulk_delete_documents`, `bulk_delete_by_external_ids`,
    `list_documents` (`Stream<Item=Page<DocumentEntry>>`),
    `cleanup_orphans`
  - Search: `search`, `search_text`, `search_vector`, `multi_search`
  - Jobs: `switch_embedding_model`, `get_job`, `list_jobs`,
    `list_tenant_jobs`
  - Cluster: `cluster_nodes`, `cluster_shards`, `cluster_health`
  - LLM settings: `get_llm_settings`, `update_llm_settings`,
    `delete_llm_settings`
  - API keys: `create_api_key`, `list_api_keys`, `get_api_key`,
    `revoke_api_key`
- Hardened transport defaults: connect timeout, idle pool TTL,
  `tcp_nodelay`, configurable `https_only`, custom user agent.
- Honours `Retry-After` on 429/503 and applies exponential backoff
  with deterministic jitter on retryable failures.
- gzip request bodies above 64 KiB.
- Pluggable LRU + TTL response cache (opt-in via builder).
- Tokio-backed singleflight coalescing for concurrent identical
  search calls.
- `tracing`-based internal logging.

### Notes
- The server does not yet expose dedicated `/version` or
  `/v1/tenants/.../api-keys` routes — those SDK methods will surface
  `Error::NotFound` until the corresponding routes ship server-side.
- `update_index` currently returns `501 Not Implemented` from the
  server; the SDK method is provided for forward-compatibility.
