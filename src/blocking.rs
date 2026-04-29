//! Blocking (synchronous) wrapper around [`crate::Client`].
//!
//! Enabled with the `blocking` cargo feature. Uses a small dedicated
//! [`tokio`] runtime so callers can use the SDK from sync code (CLIs,
//! tests, FFI shells) without wiring an executor themselves.

use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};

use crate::client::{Client, ClientBuilder};
use crate::error::Error;
use crate::types::{
    AddDocumentsRequest, AddDocumentsResponse, BulkDeleteResponse, Chunk, ClusterHealth,
    ClusterNodesResponse, ClusterShardsResponse, CreateApiKeyRequest, CreateApiKeyResponse,
    CreateIndexRequest, CreateTenantRequest, DeleteChunksResponse, DeleteDocumentResponse, Health,
    ImportDocumentsRequest, ImportDocumentsResponse, Index, IndexStatus, Job, ListApiKeysResponse,
    ListIndexesResponse, ListJobsFilter, ListJobsResponse, ListSharedIndexesResponse,
    ListTenantsResponse, ListUserIndexesResponse, LiveIndexStats, LlmSettings, MultiSearchRequest,
    MultiSearchResponse, PendingStatus, Ready, SearchRequest, SearchResponse,
    SwitchEmbeddingModelRequest, SwitchEmbeddingModelResponse, SyncDocumentsRequest,
    SyncDocumentsResponse, Tenant, UpdateIndexRequest, UpsertResourceRequest,
    UpsertResourceResponse, VersionInfo,
};

/// Synchronous client built on top of an internal Tokio runtime.
///
/// Cheap to clone; clones share the same runtime + underlying [`Client`].
#[derive(Clone)]
pub struct BlockingClient {
    client: Client,
    runtime: Arc<Runtime>,
}

impl BlockingClient {
    /// Construct a [`BlockingClient`] from an async [`Client`].
    ///
    /// Spawns a dedicated multi-thread runtime sized for the typical
    /// workload of mixing search / ingest calls (4 worker threads). For
    /// finer control, build the runtime yourself and pass it via
    /// [`BlockingClient::with_runtime`].
    pub fn new(client: Client) -> Result<Self, Error> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("graphann-blocking")
            .build()
            .map_err(|e| Error::Builder(format!("blocking runtime build failed: {e}")))?;
        Ok(Self {
            client,
            runtime: Arc::new(runtime),
        })
    }

    /// Reuse a pre-built runtime — handy for tests that already own one.
    pub fn with_runtime(client: Client, runtime: Arc<Runtime>) -> Self {
        Self { client, runtime }
    }

    /// Convenience builder that returns a ready-to-use [`BlockingClient`].
    pub fn builder() -> ClientBuilder {
        Client::builder()
    }

    /// Borrow the underlying async client (e.g. when you have your own
    /// runtime and want to use the async API directly).
    pub fn async_client(&self) -> &Client {
        &self.client
    }

    fn block_on<F, T>(&self, fut: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(fut)
    }
}

macro_rules! impl_blocking_methods {
    ($($(#[$attr:meta])* $vis:vis fn $name:ident(&self $(, $arg:ident : $ty:ty)*) -> Result<$ret:ty, Error> => $async_fn:ident);* $(;)?) => {
        impl BlockingClient {
            $(
                $(#[$attr])*
                $vis fn $name(&self, $($arg : $ty),*) -> Result<$ret, Error> {
                    let client = self.client.clone();
                    self.block_on(async move { client.$async_fn($($arg),*).await })
                }
            )*
        }
    };
}

impl_blocking_methods! {
    /// Sync wrapper for [`crate::Client::health`].
    pub fn health(&self) -> Result<Health, Error> => health;
    /// Sync wrapper for [`crate::Client::ready`].
    pub fn ready(&self) -> Result<Ready, Error> => ready;
    /// Sync wrapper for [`crate::Client::version`].
    pub fn version(&self) -> Result<VersionInfo, Error> => version;
    /// Sync wrapper for [`crate::Client::list_tenants`].
    pub fn list_tenants(&self) -> Result<ListTenantsResponse, Error> => list_tenants;
    /// Sync wrapper for [`crate::Client::create_tenant`].
    pub fn create_tenant(&self, req: CreateTenantRequest) -> Result<Tenant, Error> => create_tenant;
    /// Sync wrapper for [`crate::Client::get_tenant`].
    pub fn get_tenant(&self, tenant_id: &str) -> Result<Tenant, Error> => get_tenant;
    /// Sync wrapper for [`crate::Client::delete_tenant`].
    pub fn delete_tenant(&self, tenant_id: &str) -> Result<(), Error> => delete_tenant;

    /// Sync wrapper for [`crate::Client::list_indexes`].
    pub fn list_indexes(&self) -> Result<ListIndexesResponse, Error> => list_indexes;
    /// Sync wrapper for [`crate::Client::create_index`].
    pub fn create_index(&self, req: CreateIndexRequest) -> Result<Index, Error> => create_index;
    /// Sync wrapper for [`crate::Client::get_index`].
    pub fn get_index(&self, index_id: &str) -> Result<Index, Error> => get_index;
    /// Sync wrapper for [`crate::Client::delete_index`].
    pub fn delete_index(&self, index_id: &str) -> Result<(), Error> => delete_index;
    /// Sync wrapper for [`crate::Client::update_index`].
    pub fn update_index(&self, index_id: &str, req: UpdateIndexRequest) -> Result<Index, Error> => update_index;
    /// Sync wrapper for [`crate::Client::get_index_status`].
    pub fn get_index_status(&self, index_id: &str) -> Result<IndexStatus, Error> => get_index_status;
    /// Sync wrapper for [`crate::Client::get_live_stats`].
    pub fn get_live_stats(&self, index_id: &str) -> Result<LiveIndexStats, Error> => get_live_stats;
    /// Sync wrapper for [`crate::Client::clear_index`].
    pub fn clear_index(&self, index_id: &str) -> Result<serde_json::Value, Error> => clear_index;
    /// Sync wrapper for [`crate::Client::compact_index`].
    pub fn compact_index(&self, index_id: &str) -> Result<serde_json::Value, Error> => compact_index;

    /// Sync wrapper for [`crate::Client::add_documents`].
    pub fn add_documents(&self, index_id: &str, req: AddDocumentsRequest) -> Result<AddDocumentsResponse, Error> => add_documents;
    /// Sync wrapper for [`crate::Client::import_documents`].
    pub fn import_documents(&self, index_id: &str, req: ImportDocumentsRequest) -> Result<ImportDocumentsResponse, Error> => import_documents;
    /// Sync wrapper for [`crate::Client::get_pending_status`].
    pub fn get_pending_status(&self, index_id: &str) -> Result<PendingStatus, Error> => get_pending_status;
    /// Sync wrapper for [`crate::Client::process_pending`].
    pub fn process_pending(&self, index_id: &str) -> Result<serde_json::Value, Error> => process_pending;
    /// Sync wrapper for [`crate::Client::clear_pending`].
    pub fn clear_pending(&self, index_id: &str) -> Result<serde_json::Value, Error> => clear_pending;
    /// Sync wrapper for [`crate::Client::get_document`].
    pub fn get_document(&self, index_id: &str, doc_id: i64) -> Result<serde_json::Value, Error> => get_document;
    /// Sync wrapper for [`crate::Client::delete_document`].
    pub fn delete_document(&self, index_id: &str, doc_id: i64) -> Result<DeleteDocumentResponse, Error> => delete_document;
    /// Sync wrapper for [`crate::Client::bulk_delete_documents`].
    pub fn bulk_delete_documents(&self, index_id: &str, ids: Vec<i64>) -> Result<BulkDeleteResponse, Error> => bulk_delete_documents;
    /// Sync wrapper for [`crate::Client::bulk_delete_by_external_ids`].
    pub fn bulk_delete_by_external_ids(&self, index_id: &str, ids: Vec<String>) -> Result<BulkDeleteResponse, Error> => bulk_delete_by_external_ids;
    /// Sync wrapper for [`crate::Client::cleanup_orphans`].
    pub fn cleanup_orphans(&self) -> Result<crate::types::CleanupOrphansResponse, Error> => cleanup_orphans;
    /// Sync wrapper for [`crate::Client::run_index_gc`].
    pub fn run_index_gc(&self, index_id: &str) -> Result<crate::types::GCResponse, Error> => run_index_gc;
    /// Sync wrapper for [`crate::Client::run_admin_gc`].
    pub fn run_admin_gc(&self) -> Result<crate::types::GCResponse, Error> => run_admin_gc;
    /// Sync wrapper for [`crate::Client::get_chunk`].
    pub fn get_chunk(&self, index_id: &str, chunk_id: i64) -> Result<Chunk, Error> => get_chunk;
    /// Sync wrapper for [`crate::Client::delete_chunks`].
    pub fn delete_chunks(&self, index_id: &str, chunk_ids: Vec<i64>) -> Result<DeleteChunksResponse, Error> => delete_chunks;

    /// Sync wrapper for [`crate::Client::search`].
    pub fn search(&self, index_id: &str, req: SearchRequest) -> Result<SearchResponse, Error> => search;

    /// Sync wrapper for [`crate::Client::switch_embedding_model`].
    pub fn switch_embedding_model(&self, index_id: &str, req: SwitchEmbeddingModelRequest) -> Result<SwitchEmbeddingModelResponse, Error> => switch_embedding_model;
    /// Sync wrapper for [`crate::Client::get_job`].
    pub fn get_job(&self, job_id: &str) -> Result<Job, Error> => get_job;
    /// Sync wrapper for [`crate::Client::list_jobs`].
    pub fn list_jobs(&self, filter: ListJobsFilter) -> Result<ListJobsResponse, Error> => list_jobs;

    /// Sync wrapper for [`crate::Client::get_cluster_nodes`].
    pub fn get_cluster_nodes(&self) -> Result<ClusterNodesResponse, Error> => get_cluster_nodes;
    /// Sync wrapper for [`crate::Client::get_cluster_shards`].
    pub fn get_cluster_shards(&self) -> Result<ClusterShardsResponse, Error> => get_cluster_shards;
    /// Sync wrapper for [`crate::Client::get_cluster_health`].
    pub fn get_cluster_health(&self) -> Result<ClusterHealth, Error> => get_cluster_health;

    /// Sync wrapper for [`crate::Client::get_llm_settings`].
    pub fn get_llm_settings(&self, org_id: &str) -> Result<LlmSettings, Error> => get_llm_settings;
    /// Sync wrapper for [`crate::Client::update_llm_settings`].
    pub fn update_llm_settings(&self, org_id: &str, settings: LlmSettings) -> Result<LlmSettings, Error> => update_llm_settings;
    /// Sync wrapper for [`crate::Client::delete_llm_settings`].
    pub fn delete_llm_settings(&self, org_id: &str) -> Result<LlmSettings, Error> => delete_llm_settings;

    /// Sync wrapper for [`crate::Client::create_api_key`].
    pub fn create_api_key(&self, req: CreateApiKeyRequest) -> Result<CreateApiKeyResponse, Error> => create_api_key;
    /// Sync wrapper for [`crate::Client::list_api_keys`].
    pub fn list_api_keys(&self) -> Result<ListApiKeysResponse, Error> => list_api_keys;
    /// Sync wrapper for [`crate::Client::revoke_api_key`].
    pub fn revoke_api_key(&self, key_id: &str) -> Result<(), Error> => revoke_api_key;

    /// Sync wrapper for [`crate::Client::list_user_indexes`].
    pub fn list_user_indexes(&self, org_id: &str, user_id: &str) -> Result<ListUserIndexesResponse, Error> => list_user_indexes;
    /// Sync wrapper for [`crate::Client::list_shared_indexes`].
    pub fn list_shared_indexes(&self, org_id: &str) -> Result<ListSharedIndexesResponse, Error> => list_shared_indexes;
}

impl BlockingClient {
    /// Sync wrapper for [`crate::Client::upsert_resource`].
    pub fn upsert_resource(
        &self,
        index_id: &str,
        resource_id: &str,
        req: UpsertResourceRequest,
    ) -> Result<UpsertResourceResponse, Error> {
        let client = self.client.clone();
        self.block_on(async move { client.upsert_resource(index_id, resource_id, req).await })
    }

    /// Sync wrapper for [`crate::Client::multi_search`]. The macro can't
    /// emit triple-arg helpers cleanly, so this one is hand-written.
    pub fn multi_search(
        &self,
        org_id: &str,
        user_id: &str,
        req: MultiSearchRequest,
    ) -> Result<MultiSearchResponse, Error> {
        let client = self.client.clone();
        self.block_on(async move { client.multi_search(org_id, user_id, req).await })
    }

    /// Sync wrapper for [`crate::Client::sync_documents`].
    pub fn sync_documents(
        &self,
        org_id: &str,
        req: SyncDocumentsRequest,
    ) -> Result<SyncDocumentsResponse, Error> {
        let client = self.client.clone();
        self.block_on(async move { client.sync_documents(org_id, req).await })
    }
}

impl std::fmt::Debug for BlockingClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockingClient")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}
