# Changelog

All notable changes to the `graphann` Rust SDK are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
