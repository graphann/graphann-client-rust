//! Quickstart example.
//!
//! Walks through the full happy path:
//!
//! 1. Build a client
//! 2. Create a tenant
//! 3. (Skipped — server has no API-key endpoint yet; placeholder)
//! 4. Ingest 10 documents
//! 5. Search
//! 6. Trigger a hot embedding-model swap
//! 7. Re-search after the swap
//!
//! Run with:
//!
//! ```bash
//! GRAPHANN_BASE_URL=http://localhost:38888 \
//! GRAPHANN_API_KEY=ak_test \
//! cargo run --example quickstart
//! ```

use std::env;
use std::time::Duration;

use graphann::{
    AddDocumentsRequest, ClientBuilder, CreateIndexRequest, CreateTenantRequest, Document, Error,
    SearchRequest, SwitchEmbeddingModelRequest, UpsertResourceRequest,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let base_url =
        env::var("GRAPHANN_BASE_URL").unwrap_or_else(|_| "http://localhost:38888".into());
    let tenant_id = env::var("GRAPHANN_TENANT_ID").unwrap_or_else(|_| String::from("t_quickstart"));
    let api_key = env::var("GRAPHANN_API_KEY").unwrap_or_else(|_| String::from("ak_quickstart"));

    let client = ClientBuilder::new()
        .base_url(&base_url)?
        .api_key(&tenant_id, &api_key)
        .timeout(Duration::from_secs(30))
        .max_retries(3)
        .build()?;

    println!("checking server health");
    let health = client.health().await?;
    println!("  -> {}", health.status);

    println!("creating tenant");
    let tenant = client
        .create_tenant(CreateTenantRequest {
            id: Some(tenant_id.clone()),
            name: "quickstart".into(),
        })
        .await?;
    println!("  -> {} ({})", tenant.id, tenant.name);

    println!("creating index");
    let index = client
        .create_index(CreateIndexRequest {
            id: Some("i_quickstart".into()),
            name: "quickstart-index".into(),
            description: Some("Demo index".into()),
            ..Default::default()
        })
        .await?;
    println!("  -> {} ({})", index.id, index.name);

    println!("upserting resource");
    let upserted = client
        .upsert_resource(
            &index.id,
            "resource-quickstart",
            UpsertResourceRequest {
                text: "GraphANN stores graph topology, not embeddings.".into(),
                metadata: [("src".into(), "quickstart".into())].into(),
            },
        )
        .await?;
    println!(
        "  -> {} op={} added={} tombstoned={}",
        upserted.resource_id, upserted.operation, upserted.chunks_added, upserted.chunks_tombstoned
    );

    println!("ingesting 10 documents");
    let docs: Vec<Document> = (0..10)
        .map(|i| Document {
            id: Some(format!("doc-{i}")),
            text: format!("Document number {i} mentions cats, vectors, and storage savings."),
            ..Default::default()
        })
        .collect();
    let added = client
        .add_documents(&index.id, AddDocumentsRequest { documents: docs })
        .await?;
    println!("  -> added {} chunks", added.added);

    println!("waiting for index to settle");
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("searching");
    let results = client
        .search(
            &index.id,
            SearchRequest {
                query: Some("cats".into()),
                k: 5,
                ..Default::default()
            },
        )
        .await?;
    println!("  -> {} results", results.total);
    for r in &results.results {
        println!("    {} ({:.3})", r.id, r.score);
    }

    println!("switching embedding model");
    match client
        .switch_embedding_model(
            &index.id,
            SwitchEmbeddingModelRequest {
                embedding_backend: "ollama".into(),
                model: "nomic-embed-text".into(),
                dimension: 768,
                endpoint_override: None,
                api_key: None,
            },
        )
        .await
    {
        Ok(job) => println!("  -> queued job {}", job.job_id),
        Err(e) => println!("  -> swap unavailable: {e}"),
    }

    // Invalidate the SDK's response cache so post-swap searches don't
    // hand back stale embeddings (no-op if caching is disabled).
    client.invalidate_cache();

    println!("re-searching after swap");
    let results = client
        .search(
            &index.id,
            SearchRequest {
                query: Some("storage savings".into()),
                k: 5,
                ..Default::default()
            },
        )
        .await?;
    println!("  -> {} results", results.total);

    Ok(())
}
