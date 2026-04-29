//! Search methods on [`crate::Client`].
//!
//! This module wires the cache + single-flight optimisations on top of
//! the per-endpoint HTTP send. Both layers are opt-in via builder hooks
//! ([`crate::ClientBuilder::cache`] and [`crate::ClientBuilder::singleflight`]).

use std::sync::Arc;

use reqwest::Method;
use tracing::trace;

use crate::client::Client;
use crate::error::Error;
use crate::types::{MultiSearchRequest, MultiSearchResponse, SearchRequest, SearchResponse};

impl Client {
    /// `POST /v1/tenants/{tenantID}/indexes/{indexID}/search`.
    ///
    /// Honours the optional response cache and singleflight coalescer.
    pub async fn search(
        &self,
        index_id: &str,
        req: SearchRequest,
    ) -> Result<SearchResponse, Error> {
        let tenant = self.require_tenant()?.to_string();
        let path = format!("v1/tenants/{}/indexes/{}/search", tenant, index_id);
        self.cached_search(&path, req).await
    }

    /// `POST /v1/orgs/{orgID}/users/{userID}/search` — multi-source
    /// search across an org's indexes with RBAC filtering.
    pub async fn multi_search(
        &self,
        org_id: &str,
        user_id: &str,
        req: MultiSearchRequest,
    ) -> Result<MultiSearchResponse, Error> {
        let path = format!("v1/orgs/{}/users/{}/search", org_id, user_id);
        self.request_json(Method::POST, &path, Some(&req)).await
    }

    async fn cached_search(&self, path: &str, req: SearchRequest) -> Result<SearchResponse, Error> {
        let cache_key = format!(
            "{path}|{}",
            serde_json::to_string(&req).map_err(Error::Decode)?
        );

        // Fast-path: cache hit.
        if let Some(cache) = self.cache() {
            if let Some(hit) = cache.get(&cache_key) {
                trace!(%path, "search cache hit");
                #[cfg(feature = "metrics")]
                self.report_cache(true);
                return Ok((*hit).clone());
            }
            #[cfg(feature = "metrics")]
            self.report_cache(false);
        }

        // Coalesce concurrent identical calls.
        if let Some(sf) = self.singleflight() {
            let key = cache_key.clone();
            let client = self.clone();
            let req_for_call = req.clone();
            let path_owned = path.to_string();
            let result = sf
                .do_call(key, || async move {
                    client
                        .request_json::<_, SearchResponse>(
                            Method::POST,
                            &path_owned,
                            Some(&req_for_call),
                        )
                        .await
                        .map(Arc::new)
                        .map_err(|e| e.to_string())
                })
                .await;
            return match result {
                Ok(arc) => {
                    if let Some(cache) = self.cache() {
                        cache.put(cache_key, arc.clone());
                    }
                    Ok((*arc).clone())
                }
                Err(msg) => Err(Error::Other(msg)),
            };
        }

        let resp: SearchResponse = self.request_json(Method::POST, path, Some(&req)).await?;
        if let Some(cache) = self.cache() {
            cache.put(cache_key, Arc::new(resp.clone()));
        }
        Ok(resp)
    }

    #[cfg(feature = "metrics")]
    fn report_cache(&self, hit: bool) {
        if let Some(hook) = self.inner.metrics.read().clone() {
            hook.on_cache(hit);
        }
    }
}
