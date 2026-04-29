//! End-to-end integration tests.
//!
//! - The unit-style tests in this file run by default and use [`wiremock`]
//!   to stand in for a real server.
//! - The single `#[ignore]`-gated test at the bottom (`live_smoke`) runs
//!   against a real GraphANN server when `GRAPHANN_BASE_URL` and
//!   `GRAPHANN_API_KEY` are exported. Run it with
//!   `cargo test -- --ignored live_smoke`.

mod common;

use std::time::Duration;

use graphann::{
    AddDocumentsRequest, ApiError, ClientBuilder, CreateIndexRequest, CreateTenantRequest,
    Document, Error, ListJobsFilter, LlmSettings, SearchFilter, SearchRequest,
    SwitchEmbeddingModelRequest, SyncDocument, SyncDocumentsRequest, UpsertResourceRequest,
};
use http::header::HeaderName;
use serde_json::json;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, ResponseTemplate};

use common::fixture;

#[tokio::test]
async fn health_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .and(header("X-Tenant-ID", "t_test"))
        .and(header("X-API-Key", "ak_test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "healthy"})))
        .mount(&server)
        .await;

    let h = client.health().await.unwrap();
    assert_eq!(h.status, "healthy");
}

#[tokio::test]
async fn create_tenant_serialises_body() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants"))
        .and(header_exists(HeaderName::from_static("content-type")))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "t_xyz",
            "name": "demo",
            "created_at": "2026-04-25T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let t = client
        .create_tenant(CreateTenantRequest {
            id: None,
            name: "demo".into(),
        })
        .await
        .unwrap();
    assert_eq!(t.id, "t_xyz");
}

#[tokio::test]
async fn create_index_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "i_abc",
            "tenant_id": "t_test",
            "name": "demo",
            "status": "empty",
            "num_docs": 0,
            "num_chunks": 0,
            "dimension": 0
        })))
        .mount(&server)
        .await;

    let idx = client
        .create_index(CreateIndexRequest {
            name: "demo".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(idx.id, "i_abc");
}

#[tokio::test]
async fn search_returns_results() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"id": "chunk-1", "text": "alpha", "score": 0.9}
            ],
            "total": 1
        })))
        .mount(&server)
        .await;

    let resp = client
        .search(
            "i_abc",
            SearchRequest {
                query: Some("alpha".into()),
                k: 5,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 1);
    assert_eq!(resp.total, 1);
}

#[tokio::test]
async fn errors_map_status_to_variants() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/tenants/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {"code": "not_found", "message": "Tenant not found"}
        })))
        .mount(&server)
        .await;

    let err = client.get_tenant("missing").await.unwrap_err();
    matches!(err, Error::NotFound(_));
}

#[tokio::test]
async fn rate_limit_honours_retry_after() {
    let (server, client) = fixture().await;
    // First two responses are 429, then 200. Retry headers expressed in seconds.
    let body = json!({"error": {"code": "rate_limited", "message": "slow down"}});
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/search"))
        .respond_with(
            ResponseTemplate::new(429)
                .set_body_json(body.clone())
                .insert_header("Retry-After", "0"),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [],
            "total": 0
        })))
        .mount(&server)
        .await;

    let resp = client
        .search(
            "i_abc",
            SearchRequest {
                query: Some("hi".into()),
                k: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.results.len(), 0);
}

#[tokio::test]
async fn list_documents_streams_pages() {
    use futures::TryStreamExt;
    let (server, client) = fixture().await;

    Mock::given(method("GET"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/documents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "documents": [{"id": "doc-1", "text": "alpha"}],
            "next_cursor": "c1"
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/documents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "documents": [{"id": "doc-2", "text": "beta"}]
        })))
        .mount(&server)
        .await;

    let mut stream = client.list_documents("i_abc");
    let mut total = 0usize;
    while let Some(page) = stream.try_next().await.unwrap() {
        total += page.items.len();
    }
    assert_eq!(total, 2);
}

#[tokio::test]
async fn switch_embedding_model_returns_job_id() {
    let (server, client) = fixture().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/embedding-model"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "job_id": "job_demo",
            "status": "queued"
        })))
        .mount(&server)
        .await;

    let resp = client
        .switch_embedding_model(
            "i_abc",
            SwitchEmbeddingModelRequest {
                embedding_backend: "ollama".into(),
                model: "nomic-embed-text".into(),
                dimension: 768,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.job_id, "job_demo");
}

#[tokio::test]
async fn list_jobs_filters_propagate_as_query() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/jobs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jobs": [],
            "total": 0
        })))
        .mount(&server)
        .await;
    let _ = client
        .list_jobs(ListJobsFilter {
            limit: Some(50),
            ..Default::default()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn cluster_health_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/cluster/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok",
            "cluster_size": 3,
            "alive_nodes": 3,
            "raft_has_leader": true,
            "under_replicated_shards": 0
        })))
        .mount(&server)
        .await;
    let h = client.get_cluster_health().await.unwrap();
    assert_eq!(h.status, "ok");
    assert_eq!(h.cluster_size, 3);
}

#[tokio::test]
async fn payload_too_large_maps_to_typed_error() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/documents"))
        .respond_with(ResponseTemplate::new(413).set_body_json(json!({
            "error": {"code": "payload_too_large", "message": "Request body too large"}
        })))
        .mount(&server)
        .await;
    let docs = AddDocumentsRequest {
        documents: vec![Document {
            text: "x".repeat(2_000_000),
            ..Default::default()
        }],
    };
    let err = client.add_documents("i_abc", docs).await.unwrap_err();
    matches!(err, Error::PayloadTooLarge(_));
}

#[tokio::test]
async fn api_error_envelope_round_trip() {
    let body = json!({"code": "validation_error", "message": "k must be > 0"});
    let parsed: ApiError = serde_json::from_value(body).unwrap();
    assert_eq!(parsed.code, "validation_error");
}

#[tokio::test]
async fn ready_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/ready"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "ready"})))
        .mount(&server)
        .await;
    let r = client.ready().await.unwrap();
    assert_eq!(r.status, "ready");
}

#[tokio::test]
async fn get_chunk_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/chunks/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "chunk_id": 42,
            "text": "hello",
            "document_id": 7,
            "chunk_index": 0,
            "start": 0,
            "end": 5,
        })))
        .mount(&server)
        .await;
    let chunk = client.get_chunk("i_abc", 42).await.unwrap();
    assert_eq!(chunk.chunk_id, 42);
    assert_eq!(chunk.text, "hello");
    assert_eq!(chunk.document_id, 7);
    assert_eq!(chunk.end, 5);
}

#[tokio::test]
async fn delete_chunks_round_trip() {
    let (server, client) = fixture().await;
    // Path id is a sentinel `0`; chunk_ids ride in the body.
    Mock::given(method("DELETE"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/chunks/0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "index_id": "i_abc",
            "deleted": 3,
        })))
        .mount(&server)
        .await;
    let resp = client
        .delete_chunks("i_abc", vec![9, 10, 11])
        .await
        .unwrap();
    assert_eq!(resp.index_id, "i_abc");
    assert_eq!(resp.deleted, 3);
}

#[tokio::test]
async fn clear_pending_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/pending"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "index_id": "i_abc",
            "status": "cleared",
            "message": "Pending documents cleared",
        })))
        .mount(&server)
        .await;
    let v = client.clear_pending("i_abc").await.unwrap();
    assert_eq!(v["index_id"], "i_abc");
    assert_eq!(v["status"], "cleared");
}

#[tokio::test]
async fn process_pending_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/process"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "index_id": "i_abc",
            "processed": 3,
            "chunks_created": 5,
            "chunk_ids": [1, 2, 3, 4, 5],
        })))
        .mount(&server)
        .await;
    let v = client.process_pending("i_abc").await.unwrap();
    assert_eq!(v["processed"], 3);
    assert_eq!(v["chunks_created"], 5);
}

#[tokio::test]
async fn list_user_indexes_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/orgs/org_demo/users/u_alice/indexes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "indexes": [
                {
                    "id": "i_personal",
                    "tenant_id": "t_org_demo",
                    "name": "github",
                    "status": "ready",
                    "num_docs": 12,
                    "num_chunks": 88,
                    "dimension": 768,
                    "path": "org/org_demo/users/u_alice/github",
                }
            ],
            "total": 1,
            "org_id": "org_demo",
            "user_id": "u_alice",
        })))
        .mount(&server)
        .await;
    let resp = client
        .list_user_indexes("org_demo", "u_alice")
        .await
        .unwrap();
    assert_eq!(resp.total, 1);
    assert_eq!(resp.indexes.len(), 1);
    assert_eq!(resp.indexes[0].id, "i_personal");
    assert_eq!(resp.user_id, "u_alice");
}

#[tokio::test]
async fn list_shared_indexes_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/orgs/org_demo/shared/indexes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "indexes": [
                {
                    "id": "i_shared",
                    "tenant_id": "t_org_demo",
                    "name": "confluence",
                    "status": "ready",
                    "num_docs": 200,
                    "num_chunks": 1500,
                    "dimension": 768,
                    "path": "org/org_demo/shared/confluence",
                }
            ],
            "total": 1,
            "org_id": "org_demo",
        })))
        .mount(&server)
        .await;
    let resp = client.list_shared_indexes("org_demo").await.unwrap();
    assert_eq!(resp.total, 1);
    assert_eq!(resp.indexes[0].id, "i_shared");
    assert_eq!(resp.org_id, "org_demo");
}

#[tokio::test]
async fn sync_documents_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/orgs/org_demo/documents"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "synced": 2,
            "org_id": "org_demo",
            "user_id": "u_alice",
            "source_type": "github",
            "index_type": "shared",
        })))
        .mount(&server)
        .await;
    let req = SyncDocumentsRequest {
        user_id: "u_alice".into(),
        source_type: "github".into(),
        shared: true,
        documents: vec![
            SyncDocument {
                resource_id: Some("r_1".into()),
                text: "alpha".into(),
                metadata: None,
            },
            SyncDocument {
                resource_id: Some("r_2".into()),
                text: "beta".into(),
                metadata: None,
            },
        ],
    };
    let resp = client.sync_documents("org_demo", req).await.unwrap();
    assert_eq!(resp.synced, 2);
    assert_eq!(resp.index_type, "shared");
    assert_eq!(resp.org_id, "org_demo");
}

#[tokio::test]
async fn llm_settings_get_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("GET"))
        .and(path("/v1/orgs/org_demo/llm-settings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "provider": "openai",
            "model": "gpt-4",
            "api_key": "***abcd",
        })))
        .mount(&server)
        .await;
    let s = client.get_llm_settings("org_demo").await.unwrap();
    assert_eq!(s.provider, "openai");
    assert_eq!(s.model, "gpt-4");
    assert_eq!(s.api_key.as_deref(), Some("***abcd"));
}

#[tokio::test]
async fn llm_settings_update_uses_patch() {
    let (server, client) = fixture().await;
    Mock::given(method("PATCH"))
        .and(path("/v1/orgs/org_demo/llm-settings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "provider": "ollama",
            "model": "llama3",
            "api_key": "***xyz",
            "temperature": 0.2,
        })))
        .mount(&server)
        .await;
    let merged = client
        .update_llm_settings(
            "org_demo",
            LlmSettings {
                provider: "ollama".into(),
                model: "llama3".into(),
                temperature: Some(0.2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(merged.provider, "ollama");
    assert_eq!(merged.model, "llama3");
    assert_eq!(merged.temperature, Some(0.2));
}

#[tokio::test]
async fn llm_settings_delete_returns_settings() {
    let (server, client) = fixture().await;
    Mock::given(method("DELETE"))
        .and(path("/v1/orgs/org_demo/llm-settings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "provider": "ollama",
            "model": "llama3",
        })))
        .mount(&server)
        .await;
    let defaults = client.delete_llm_settings("org_demo").await.unwrap();
    assert_eq!(defaults.provider, "ollama");
    assert_eq!(defaults.model, "llama3");
}

#[tokio::test]
async fn upsert_resource_create() {
    let (server, client) = fixture().await;
    Mock::given(method("PUT"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/resources/doc-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource_id": "doc-1",
            "chunks_added": 3,
            "chunks_tombstoned": 0,
            "operation": "create",
        })))
        .mount(&server)
        .await;
    let resp = client
        .upsert_resource(
            "i_abc",
            "doc-1",
            UpsertResourceRequest {
                text: "hello world".into(),
                metadata: [("src".into(), "test".into())].into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.resource_id, "doc-1");
    assert_eq!(resp.chunks_added, 3);
    assert_eq!(resp.chunks_tombstoned, 0);
    assert_eq!(resp.operation, "create");
}

#[tokio::test]
async fn upsert_resource_update() {
    let (server, client) = fixture().await;
    Mock::given(method("PUT"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/resources/doc-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource_id": "doc-1",
            "chunks_added": 2,
            "chunks_tombstoned": 3,
            "operation": "update",
        })))
        .mount(&server)
        .await;
    let resp = client
        .upsert_resource(
            "i_abc",
            "doc-1",
            UpsertResourceRequest {
                text: "updated".into(),
                metadata: Default::default(),
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.operation, "update");
    assert_eq!(resp.chunks_tombstoned, 3);
}

#[tokio::test]
async fn create_index_with_compression_and_approximate() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "i_pq",
            "tenant_id": "t_test",
            "name": "pq-index",
            "status": "empty",
            "num_docs": 0,
            "num_chunks": 0,
            "dimension": 0,
            "compression": "pq",
            "approximate": true,
        })))
        .mount(&server)
        .await;
    let idx = client
        .create_index(CreateIndexRequest {
            name: "pq-index".into(),
            compression: Some("pq".into()),
            approximate: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(idx.id, "i_pq");
    assert_eq!(idx.compression.as_deref(), Some("pq"));
    assert_eq!(idx.approximate, Some(true));
}

#[tokio::test]
async fn search_filter_equals_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [],
            "total": 0,
        })))
        .mount(&server)
        .await;
    let resp = client
        .search(
            "i_abc",
            SearchRequest {
                query: Some("hello".into()),
                filter: SearchFilter {
                    equals: [("lang".into(), "en".into())].into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.total, 0);
}

#[tokio::test]
#[ignore]
async fn live_smoke() {
    let base_url = match std::env::var("GRAPHANN_BASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipping live_smoke (GRAPHANN_BASE_URL not set)");
            return;
        }
    };
    let api_key =
        std::env::var("GRAPHANN_API_KEY").expect("GRAPHANN_API_KEY required for live test");
    let tenant_id = std::env::var("GRAPHANN_TENANT_ID").unwrap_or_else(|_| "t_smoke".into());
    let client = ClientBuilder::new()
        .base_url(&base_url)
        .unwrap()
        .api_key(&tenant_id, &api_key)
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let h = client.health().await.unwrap();
    assert_eq!(h.status, "healthy");
}
