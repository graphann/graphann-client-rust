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
    Document, Error, ListJobsFilter, LlmSettings, SearchFilter, SearchRequest, SearchResponse,
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
        ..Default::default()
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
async fn cleanup_orphans_default_omits_query_params() {
    use wiremock::matchers::query_param_is_missing;

    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/admin/cleanup-orphans"))
        .and(query_param_is_missing("min_age"))
        .and(query_param_is_missing("dry_run"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "removed": ["/data/tenants/t/indexes/i.compact"],
            "freed_bytes": 4096,
            "min_age": "1h0m0s",
            "dry_run": false,
        })))
        .mount(&server)
        .await;

    let resp = client
        .cleanup_orphans(Duration::from_secs(0), false)
        .await
        .unwrap();
    assert_eq!(resp.freed_bytes, 4096);
    assert_eq!(resp.removed.len(), 1);
    assert_eq!(resp.min_age, "1h0m0s");
    assert!(!resp.dry_run);
}

#[tokio::test]
async fn cleanup_orphans_passes_min_age_and_dry_run() {
    use wiremock::matchers::query_param;

    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/admin/cleanup-orphans"))
        .and(query_param("min_age", "24h0m0s"))
        .and(query_param("dry_run", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "removed": ["/data/tenants/t/indexes/i.pre-reembed.20260101T000000Z"],
            "freed_bytes": 0,
            "min_age": "24h0m0s",
            "dry_run": true,
        })))
        .mount(&server)
        .await;

    let resp = client
        .cleanup_orphans(Duration::from_secs(24 * 3600), true)
        .await
        .unwrap();
    assert!(resp.dry_run);
    assert_eq!(resp.min_age, "24h0m0s");
    assert_eq!(resp.removed.len(), 1);
}

// --- ingest options + precomputed vectors (v0.7.0) -------------------------

/// Per-document `vector` rides the wire on `add_documents`. Values are
/// chosen to be exactly representable in f32 so the f32→f64 JSON
/// round-trip stays bit-identical for the partial-body match.
#[tokio::test]
async fn add_documents_precomputed_vectors_serialise() {
    use wiremock::matchers::body_partial_json;

    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/documents"))
        .and(body_partial_json(json!({
            "documents": [
                {"id": "doc-1", "text": "alpha", "vector": [0.5, 0.25, -1.0]}
            ]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "added": 1,
            "index_id": "i_abc",
            "chunk_ids": ["chunk-1"]
        })))
        .mount(&server)
        .await;

    let resp = client
        .add_documents(
            "i_abc",
            AddDocumentsRequest {
                documents: vec![Document {
                    id: Some("doc-1".into()),
                    text: "alpha".into(),
                    vector: Some(vec![0.5, 0.25, -1.0]),
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.added, 1);
    assert_eq!(resp.external_ids, None);
}

/// `defer_save` / `bulk` serialise as body fields when set.
#[tokio::test]
async fn add_documents_defer_save_and_bulk_flags_serialise() {
    use wiremock::matchers::body_partial_json;

    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/documents"))
        .and(body_partial_json(json!({"defer_save": true, "bulk": true})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "added": 2,
            "index_id": "i_abc",
            "chunk_ids": ["chunk-1", "chunk-2"]
        })))
        .mount(&server)
        .await;

    let resp = client
        .add_documents(
            "i_abc",
            AddDocumentsRequest {
                documents: vec![
                    Document {
                        text: "alpha".into(),
                        ..Default::default()
                    },
                    Document {
                        text: "beta".into(),
                        ..Default::default()
                    },
                ],
                defer_save: true,
                bulk: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.added, 2);
}

/// Default `AddDocumentsRequest` serialisation is byte-compatible with
/// pre-0.7 SDKs: no `defer_save` / `bulk` / `vector` keys. Pins the wire
/// shape because the server decodes with `DisallowUnknownFields` and
/// older servers would 400 on unexpected keys.
#[tokio::test]
async fn add_documents_default_wire_shape_unchanged() {
    let req = AddDocumentsRequest {
        documents: vec![Document {
            text: "a".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let v = serde_json::to_value(&req).unwrap();
    assert_eq!(v, json!({"documents": [{"text": "a"}]}));
}

/// `external_ids` decodes when the server minted ids (sharded ingest of
/// id-less documents); positionally aligned with the request array.
#[tokio::test]
async fn add_documents_response_external_ids_decode() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/documents"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "added": 2,
            "index_id": "i_abc",
            "chunk_ids": ["chunk-1", "chunk-2"],
            "external_ids": ["minted-1", "client-2"]
        })))
        .mount(&server)
        .await;

    let resp = client
        .add_documents(
            "i_abc",
            AddDocumentsRequest {
                documents: vec![
                    Document {
                        text: "alpha".into(),
                        ..Default::default()
                    },
                    Document {
                        id: Some("client-2".into()),
                        text: "beta".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        resp.external_ids,
        Some(vec!["minted-1".to_string(), "client-2".to_string()])
    );
}

/// `flush_index` POSTs `{}` with `Content-Type: application/json` (the
/// server's middleware rejects body-less POSTs without the header) and
/// decodes `{"flushed": true}`.
#[tokio::test]
async fn flush_index_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/flush"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"flushed": true})))
        .mount(&server)
        .await;

    let resp = client.flush_index("i_abc").await.unwrap();
    assert!(resp.flushed);
}

/// `rebuild_graph` round trip — migration endpoint for pre-2026-06
/// fragmented delta graphs.
#[tokio::test]
async fn rebuild_graph_round_trip() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/rebuild-graph"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "rebuilt": true,
            "chunks": 52000,
            "wall_ms": 1234
        })))
        .mount(&server)
        .await;

    let resp = client.rebuild_graph("i_abc").await.unwrap();
    assert!(resp.rebuilt);
    assert_eq!(resp.chunks, 52000);
    assert_eq!(resp.wall_ms, 1234);
}

/// `rebuild_graph` while a compaction is running surfaces the server's
/// 409 as the typed `Error::Conflict`.
#[tokio::test]
async fn rebuild_graph_conflict_maps_to_typed_error() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/rebuild-graph"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({
            "error": {"code": "conflict", "message": "compaction already in progress for this index"}
        })))
        .mount(&server)
        .await;

    let err = client.rebuild_graph("i_abc").await.unwrap_err();
    assert!(matches!(err, Error::Conflict(_)));
}

// --- ef_search + sharded search response (v0.7.0) ---------------------------

/// `ef_search` serialises when set and is omitted entirely by default
/// (0/omitted = server default, `--search-ef`).
#[tokio::test]
async fn search_ef_search_serialises_and_default_omits() {
    use wiremock::matchers::body_partial_json;

    let v = serde_json::to_value(SearchRequest::default()).unwrap();
    assert!(v.get("ef_search").is_none());

    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/search"))
        .and(body_partial_json(json!({"ef_search": 128})))
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
                query: Some("hello".into()),
                k: 5,
                ef_search: Some(128),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.total, 0);
}

/// Sharded scatter-gather responses carry the additive partial-results
/// keys; `degraded_shards` is present only when non-empty.
#[tokio::test]
async fn search_response_sharded_fields_decode() {
    let (server, client) = fixture().await;
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_abc/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "chunk-1", "text": "alpha", "score": 0.9}],
            "total": 1,
            "partial": true,
            "shards_total": 4,
            "shards_ok": 3,
            "degraded_shards": ["shard-2"]
        })))
        .mount(&server)
        .await;

    let resp = client
        .search(
            "i_abc",
            SearchRequest {
                query: Some("alpha".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(resp.total, 1);
    assert_eq!(resp.partial, Some(true));
    assert_eq!(resp.shards_total, Some(4));
    assert_eq!(resp.shards_ok, Some(3));
    assert_eq!(resp.degraded_shards, Some(vec!["shard-2".to_string()]));
}

/// Local (non-sharded) search responses stay exactly `{results, total}`
/// — every sharded field decodes as `None`.
#[tokio::test]
async fn search_response_local_path_omits_sharded_fields() {
    let resp: SearchResponse = serde_json::from_value(json!({"results": [], "total": 0})).unwrap();
    assert_eq!(resp.partial, None);
    assert_eq!(resp.shards_total, None);
    assert_eq!(resp.shards_ok, None);
    assert_eq!(resp.degraded_shards, None);
}

// --- gzip request-body tests ----------------------------------------------
//
// graphann's HTTP server does not decode `Content-Encoding: gzip` request
// bodies. The SDK's auto-gzip is therefore opt-in
// (`ClientBuilder::compress_requests(true)`). These tests pin the
// behaviour: large bodies are sent uncompressed by default; only the
// explicit opt-in produces a gzipped wire body.

/// Default builder must NOT gzip request bodies, even ones above the
/// threshold. Regression for the silent 400 "Invalid JSON body"
/// failures observed against stock graphann when the SDK auto-gzipped
/// large /documents batches.
///
/// Pins the absence of `Content-Encoding: gzip` two ways: a positive
/// match on `Content-Type: application/json` (sent on every body) and
/// a `body_partial_json` match that would fail for a gzipped wire body.
#[tokio::test]
async fn default_client_does_not_gzip_large_request_bodies() {
    let (server, client) = fixture().await;
    // body_string_contains matches the raw uncompressed JSON; if the SDK
    // had gzipped the body, the wire bytes would start with the gzip magic
    // 0x1f 0x8b and the substring would NOT be present, so the mock would
    // 404 and add_documents would fail — exactly the regression we're
    // pinning.
    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_test/documents"))
        .and(header("content-type", "application/json"))
        .and(wiremock::matchers::body_string_contains("\"documents\":"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "added": 1,
            "index_id": "i_test",
            "chunk_ids": ["chunk-1"]
        })))
        .mount(&server)
        .await;

    let big_text = "abcdefghij".repeat(8 * 1024); // 80 KiB body
    let req = AddDocumentsRequest {
        documents: vec![Document {
            text: big_text,
            ..Default::default()
        }],
        ..Default::default()
    };
    let resp = client
        .add_documents("i_test", req)
        .await
        .expect("default-config large POST must succeed without gzip");
    assert_eq!(resp.added, 1);
}

/// `compress_requests(true)` must emit `Content-Encoding: gzip` for
/// bodies above the threshold. Opt-in path for callers behind a proxy
/// that decompresses before forwarding.
#[tokio::test]
async fn compress_requests_opt_in_sets_gzip_header() {
    let server = wiremock::MockServer::start().await;
    let client = ClientBuilder::new()
        .base_url(server.uri())
        .unwrap()
        .api_key("t_test", "ak_test")
        .timeout(Duration::from_secs(5))
        .max_retries(0)
        .compress_requests(true)
        .build()
        .unwrap();

    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes/i_test/documents"))
        .and(header("content-encoding", "gzip"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "added": 1,
            "index_id": "i_test",
            "chunk_ids": ["chunk-1"]
        })))
        .mount(&server)
        .await;

    let big_text = "abcdefghij".repeat(8 * 1024);
    let req = AddDocumentsRequest {
        documents: vec![Document {
            text: big_text,
            ..Default::default()
        }],
        ..Default::default()
    };
    let resp = client
        .add_documents("i_test", req)
        .await
        .expect("opt-in gzip POST must reach the matched mock");
    assert_eq!(resp.added, 1);
}

/// Even with `compress_requests(true)`, small bodies stay uncompressed
/// because they're below the threshold. Pins the threshold semantics so
/// callers don't see surprise gzip on tiny calls.
#[tokio::test]
async fn compress_requests_opt_in_skips_small_bodies() {
    let server = wiremock::MockServer::start().await;
    let client = ClientBuilder::new()
        .base_url(server.uri())
        .unwrap()
        .api_key("t_test", "ak_test")
        .timeout(Duration::from_secs(5))
        .max_retries(0)
        .compress_requests(true)
        .build()
        .unwrap();

    Mock::given(method("POST"))
        .and(path("/v1/tenants/t_test/indexes"))
        .and(header("content-type", "application/json"))
        .and(wiremock::matchers::body_partial_json(json!({
            "name": "tiny"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": "i_abc",
            "tenant_id": "t_test",
            "name": "tiny",
            "status": "empty",
            "num_docs": 0,
            "num_chunks": 0,
            "dimension": 0
        })))
        .mount(&server)
        .await;

    client
        .create_index(CreateIndexRequest {
            name: "tiny".into(),
            ..Default::default()
        })
        .await
        .expect("small body must skip gzip even when opt-in is set");
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
