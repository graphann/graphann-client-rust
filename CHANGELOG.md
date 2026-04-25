# Changelog

All notable changes to the `graphann` Rust SDK are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
