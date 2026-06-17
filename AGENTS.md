# AGENTS.md — graphann Rust SDK

LLM-usage guide for coding agents driving the `graphann` crate. Every
snippet below uses real method names and field names from `src/`. Do not
invent methods. When unsure, check `src/types.rs` (wire types) and the
per-module method files (`src/{tenants,indexes,documents,search,apikey}.rs`).

## Install

```toml
[dependencies]
graphann = "0.8"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Default TLS is `rustls`. Use the `native-tls` feature for the host stack,
`blocking` for `BlockingClient`, `metrics` for the `MetricsHook`.

## Client construction and auth

`ClientBuilder` is the only constructor. `base_url` is fallible (URL
parse); `api_key(tenant_id, api_key)` sets the tenant scope used by every
tenant-scoped call plus the `X-Tenant-ID` / `X-API-Key` headers.

```rust
use std::time::Duration;
use graphann::{ClientBuilder, Client, Error};

async fn build() -> Result<Client, Error> {
    let client = ClientBuilder::new()
        .base_url("https://api.graphann.com")?
        .api_key("t_xyz789", "ak_demo")
        .timeout(Duration::from_secs(30))
        .max_retries(3)
        .build()?;
    Ok(client)
}
```

Tenant-scoped methods (`create_index`, `add_documents`, `search`, the
api-key calls, ...) read the tenant from `api_key(...)`. Calling one
without a tenant set returns an error from `require_tenant`. Org-scoped
methods (`multi_search`, `list_user_indexes`, `sync_documents`) take the
org/user ids as explicit arguments instead.

## Create tenant -> index -> ingest -> search

```rust
use graphann::{
    AddDocumentsRequest, CreateIndexRequest, CreateTenantRequest, Document,
    SearchRequest,
};

# async fn run(client: graphann::Client) -> Result<(), graphann::Error> {
// Tenant (id optional; when set the create is idempotent).
let tenant = client
    .create_tenant(CreateTenantRequest { id: None, name: "demo".into() })
    .await?;

// Index. compression: "none" | "scalar" | "binary" | "pq" | "recompute".
let index = client
    .create_index(CreateIndexRequest {
        name: "docs".into(),
        compression: Some("pq".into()),
        approximate: Some(true),
        ..Default::default()
    })
    .await?;

// Ingest text. The server embeds each document.
let resp = client
    .add_documents(
        &index.id,
        AddDocumentsRequest {
            documents: vec![Document {
                id: Some("doc-1".into()),
                text: "GraphANN stores graph topology, not embeddings.".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .await?;
println!("added {} chunks", resp.added);

// Search by text.
let hits = client
    .search(&index.id, SearchRequest {
        query: Some("storage savings".into()),
        k: 10,
        ..Default::default()
    })
    .await?;
for r in &hits.results {
    println!("{} score={:.3}", r.id, r.score);
}
# Ok(()) }
```

## Ingest precomputed vectors

Set `Document::vector`. When **every** document in the batch carries a
non-empty vector the server skips embedding and ingests the vectors
directly. Mixed batches (some with a vector, some without) are rejected
with HTTP 400 (`Error::Server` / validation). Vector length must match the
index dimension once it is fixed; a fresh index (dimension 0) accepts any
length and the first ingest fixes it.

```rust
use graphann::{AddDocumentsRequest, Document};

# async fn run(client: graphann::Client, index_id: &str) -> Result<(), graphann::Error> {
let resp = client
    .add_documents(index_id, AddDocumentsRequest {
        documents: vec![Document {
            id: Some("doc-1".into()),
            text: "alpha".into(),
            vector: Some(vec![0.5, 0.25, -1.0]),
            ..Default::default()
        }],
        ..Default::default()
    })
    .await?;
# let _ = resp; Ok(()) }
```

Search also accepts a precomputed query vector via `SearchRequest::vector`
(set `query` or `vector`; both may be set for a hybrid search).

## Search: rerank and ef_search

Reranking opts in per query via `rerank` / `candidate_k` / `rerank_k`. It
is a no-op against servers without a `--reranker-url`, and only applies
when `query` (text) is supplied. `score` is always cosine similarity;
`rerank_score` is `Some(_)` only when the server actually reranked that
entry. `ef_search` is the per-query HNSW expansion factor; `None` (or
`Some(0)`) uses the server default.

```rust
use graphann::SearchRequest;

# async fn run(client: graphann::Client, index_id: &str) -> Result<(), graphann::Error> {
let resp = client
    .search(index_id, SearchRequest {
        query: Some("what does the standard say about audit trails?".into()),
        k: 10,
        rerank: true,
        candidate_k: Some(50), // HNSW pool before rerank (default max(4*k, 50))
        rerank_k: Some(10),    // post-rerank count (default k)
        ef_search: Some(128),
        ..Default::default()
    })
    .await?;
for hit in &resp.results {
    match hit.rerank_score {
        Some(s) => println!("{} rerank={:.3} cosine={:.3}", hit.id, s, hit.score),
        None => println!("{} cosine={:.3}", hit.id, hit.score),
    }
}
# Ok(()) }
```

Sharded (cluster) responses additionally populate `partial`,
`shards_total`, `shards_ok`, and `degraded_shards`; all are `None` on
single-node / single-shard deployments. Note: rerank / candidate_k /
rerank_k are not applied on the sharded path.

## Bulk ingest: defer_save / bulk + flush_index

`defer_save` skips the per-batch full-delta save (data stays in memory,
still searchable) until a flush. `bulk` implies `defer_save` and also
defers the per-node HNSW insert — the delta graph is built once,
concurrently, at flush. Bulk-ingested data is not searchable until then,
except the first search against a pending deferred build transparently
triggers it server-side (build-on-read).

```rust
use graphann::{AddDocumentsRequest, Document};

# async fn run(client: graphann::Client, index_id: &str, docs: Vec<Document>) -> Result<(), graphann::Error> {
client
    .add_documents(index_id, AddDocumentsRequest {
        documents: docs,
        defer_save: true,
        bulk: true,
    })
    .await?;

// Persist the in-memory delta and build the deferred graph.
let flushed = client.flush_index(index_id).await?;
assert!(flushed.flushed);
# Ok(()) }
```

`rebuild_graph(index_id)` rebuilds a fragmented delta graph (migration for
pre-2026-06 indexes); a concurrent compaction surfaces as `Error::Conflict`.

## API keys: create / list / revoke

Wire contract matches the server. Create sends `{ user_id, name }` (both
plain strings; `user_id` may be empty). The create response carries the
secret in `plaintext` and it is returned **exactly once** — persist it
immediately; the server stores only an argon2id hash. List returns the
`api_keys` wrapper; each item is `{ id, user_id, name, created_at,
last_used_at }`.

```rust
use graphann::CreateApiKeyRequest;

# async fn run(client: graphann::Client) -> Result<(), graphann::Error> {
let created = client
    .create_api_key(CreateApiKeyRequest {
        user_id: "u_alice".into(),
        name: "ci-key".into(),
    })
    .await?;
// Store this NOW — it is never returned again.
if let Some(secret) = created.plaintext {
    println!("save this key: {secret}");
}

let keys = client.list_api_keys().await?;
for k in &keys.api_keys {
    println!("{} name={:?} last_used={:?}", k.id, k.name, k.last_used_at);
}

client.revoke_api_key(&created.id).await?;
# Ok(()) }
```

These calls require Admin RBAC server-side. There is no per-key GET.

## Error handling idioms

Every fallible method returns `Result<T, graphann::Error>`. Server statuses
map to typed variants so you can branch without parsing codes; transport /
decode failures wrap the underlying error. `Error::is_retryable()` is
`true` for `RateLimit`, `ServiceUnavailable`, and `Network` (the builder's
`max_retries` already retries these with `Retry-After` honoured).

```rust
use graphann::{Error, SearchRequest};

# async fn run(client: graphann::Client, index_id: &str) {
match client.search(index_id, SearchRequest { k: 10, ..Default::default() }).await {
    Ok(resp) => println!("{} hits", resp.total),
    Err(Error::NotFound(msg)) => eprintln!("missing: {msg}"),
    Err(Error::Conflict(msg)) => eprintln!("conflict: {msg}"),
    Err(Error::PayloadTooLarge(msg)) => eprintln!("too big: {msg}"),
    Err(Error::RateLimit { retry_after, .. }) => eprintln!("slow down: {retry_after:?}"),
    Err(e) if e.is_retryable() => eprintln!("transient: {e}"),
    Err(e) => eprintln!("fatal: {e}"),
}
# }
```

## Key gotchas

- **Plaintext key returned ONCE.** `CreateApiKeyResponse::plaintext` is the
  only time the secret is exposed. Lose it and you must revoke and recreate.
- **16 MB request-body cap.** The server caps request bodies at 16 MB, so
  precomputed-vector batches top out around ~1700 docs. Oversized bodies
  surface as `Error::PayloadTooLarge`. Split large ingests.
- **delete_chunks path quirk.** `delete_chunks(index_id, chunk_ids)` sends
  the ids in the request body and the server ignores the trailing path
  segment (the SDK posts a sentinel `0`). Pass the real ids in the `Vec`,
  not the path.
- **No request gzip by default.** The stock server does not decode gzipped
  request bodies, so the SDK only gzips when you opt in with
  `ClientBuilder::compress_requests(true)`.
- **Tenant must be set** before tenant-scoped calls; `api_key(tenant, key)`
  sets it. Org-scoped calls take ids as arguments instead.
- **Blocking client.** With the `blocking` feature, `BlockingClient`
  mirrors the async surface (`create_index`, `add_documents`, `search`,
  `flush_index`, `create_api_key`, ...) with sync signatures.
</content>
</invoke>
