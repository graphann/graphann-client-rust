# graphann

Official Rust client SDK for the [GraphANN](https://graphann.com) vector
database.

GraphANN is a storage-efficient vector database; this crate is the
public client surface for the same HTTP API the official Go, Python,
and TypeScript SDKs speak. Pick whichever language fits the host
service.

## Install

```toml
[dependencies]
graphann = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The crate ships with `rustls`-backed TLS by default; if you prefer the
host's native stack, pull in the `native-tls` feature instead. Add the
`blocking` feature when you need a sync client and `metrics` to plumb
in a metrics hook.

## Quickstart

```rust,no_run
use std::time::Duration;

use graphann::{ClientBuilder, SearchRequest};

#[tokio::main]
async fn main() -> Result<(), graphann::Error> {
    let client = ClientBuilder::new()
        .base_url("https://api.graphann.com")?
        .api_key("t_xyz789", "ak_demo")
        .timeout(Duration::from_secs(30))
        .max_retries(3)
        .build()?;

    let _health = client.health().await?;

    let req = SearchRequest {
        query: Some("hello".into()),
        k: 10,
        ..Default::default()
    };
    let _results = client.search("i_abc123", req).await?;

    Ok(())
}
```

A more substantial example — tenant + index + ingest + search + hot
model swap — lives in [`examples/quickstart.rs`](examples/quickstart.rs).
Run it against a local server with:

```bash
GRAPHANN_BASE_URL=http://localhost:38888 \
GRAPHANN_API_KEY=ak_demo \
cargo run --example quickstart
```

## Cargo features

| Feature      | Default | Description                                          |
|--------------|---------|------------------------------------------------------|
| `rustls`     | yes     | TLS via rustls (no OpenSSL).                         |
| `native-tls` | no      | TLS via the host's native stack.                     |
| `blocking`   | no      | Synchronous `BlockingClient` alongside the async API.|
| `metrics`    | no      | Pluggable metrics hook (see `MetricsHook`).          |

## API surface

```text
Health        health, ready, version
Tenants       list_tenants, create_tenant, get_tenant, delete_tenant
Indexes       list_indexes, create_index, get_index, delete_index, update_index,
              clear_index, build_index, compact_index, live_index_stats, get_index_status
Documents     add_documents, import_documents, pending_status, process_pending,
              clear_pending, get_document, delete_document, bulk_delete_documents,
              bulk_delete_by_external_ids, list_documents (Stream), cleanup_orphans
Search        search, search_text, search_vector, multi_search
Jobs          switch_embedding_model, get_job, list_jobs, list_tenant_jobs
Cluster       cluster_nodes, cluster_shards, cluster_health
LLM Settings  get_llm_settings, update_llm_settings, delete_llm_settings
API Keys      create_api_key, list_api_keys, get_api_key, revoke_api_key
```

`list_documents` returns a `futures::Stream<Item = Result<Page<T>, Error>>`
so you can iterate cursors with the standard streams toolkit:

```rust,no_run
use futures::TryStreamExt;
# async fn run(client: graphann::Client) -> Result<(), graphann::Error> {
let mut stream = client.list_documents("i_abc123");
while let Some(page) = stream.try_next().await? {
    for doc in page.items {
        println!("{}", doc.id);
    }
}
# Ok(()) }
```

## Reliability and back-pressure

- Connection pooling, `tcp_nodelay`, configurable connect timeout and
  pool idle TTL out of the box.
- Honours `Retry-After` on `429 Too Many Requests` and `503 Service
  Unavailable`.
- Falls back to exponential backoff with deterministic jitter on
  retryable transport / status errors.
- Request bodies above 64 KiB are gzipped before sending.
- Optional LRU + TTL response cache, invalidated on demand via
  `client.invalidate_cache()`.
- Optional Tokio-backed singleflight coalesces identical concurrent
  search calls into one HTTP roundtrip.
- All HTTP requests carry the SDK user-agent
  `graphann-rust/<version> (rustc/<rustc>; <os>/<arch>)`.

## Logging

Internal events are emitted via [`tracing`]. Install a subscriber in
your binary (the SDK does not).

```rust
tracing_subscriber::fmt::init();
```

## License

Commercial — see [`LICENSE`](LICENSE) at the repo root for terms.

[`tracing`]: https://docs.rs/tracing
