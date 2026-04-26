//! Cluster read-only endpoints on [`crate::Client`].

use reqwest::Method;

use crate::client::Client;
use crate::error::Error;
use crate::types::{ClusterHealth, ClusterNodesResponse, ClusterShardsResponse};

impl Client {
    /// `GET /v1/cluster/nodes` — admin-only.
    pub async fn get_cluster_nodes(&self) -> Result<ClusterNodesResponse, Error> {
        self.request_json(Method::GET, "v1/cluster/nodes", Option::<&()>::None)
            .await
    }

    /// `GET /v1/cluster/shards` — admin-only.
    pub async fn get_cluster_shards(&self) -> Result<ClusterShardsResponse, Error> {
        self.request_json(Method::GET, "v1/cluster/shards", Option::<&()>::None)
            .await
    }

    /// `GET /v1/cluster/health` — unauth'd liveness probe.
    pub async fn get_cluster_health(&self) -> Result<ClusterHealth, Error> {
        self.request_json(Method::GET, "v1/cluster/health", Option::<&()>::None)
            .await
    }
}
