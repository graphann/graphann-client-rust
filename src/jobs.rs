//! Async job (hot model switch) endpoints on [`crate::Client`].

use reqwest::Method;

use crate::client::Client;
use crate::error::Error;
use crate::types::{
    Job, ListJobsFilter, ListJobsResponse, SwitchEmbeddingModelRequest,
    SwitchEmbeddingModelResponse,
};

impl Client {
    /// `PATCH /v1/tenants/{tenantID}/indexes/{indexID}/embedding-model`.
    /// Returns 202 Accepted with the newly created job id.
    pub async fn switch_embedding_model(
        &self,
        index_id: &str,
        req: SwitchEmbeddingModelRequest,
    ) -> Result<SwitchEmbeddingModelResponse, Error> {
        let tenant = self.require_tenant()?;
        let path = format!("v1/tenants/{}/indexes/{}/embedding-model", tenant, index_id);
        self.request_json(Method::PATCH, &path, Some(&req)).await
    }

    /// `GET /v1/jobs/{jobID}`. Polls a single job by id.
    pub async fn get_job(&self, job_id: &str) -> Result<Job, Error> {
        let path = format!("v1/jobs/{}", job_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `GET /v1/jobs` — admin-only listing across every tenant.
    pub async fn list_jobs(&self, filter: ListJobsFilter) -> Result<ListJobsResponse, Error> {
        let mut query = String::new();
        push_filter(&mut query, &filter);
        let path = if query.is_empty() {
            "v1/jobs".to_string()
        } else {
            format!("v1/jobs?{}", query)
        };
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `GET /v1/tenants/{tenantID}/jobs`.
    pub async fn list_tenant_jobs(
        &self,
        tenant_id: &str,
        filter: ListJobsFilter,
    ) -> Result<ListJobsResponse, Error> {
        let mut query = String::new();
        push_filter(&mut query, &filter);
        let path = if query.is_empty() {
            format!("v1/tenants/{}/jobs", tenant_id)
        } else {
            format!("v1/tenants/{}/jobs?{}", tenant_id, query)
        };
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }
}

fn push_filter(out: &mut String, filter: &ListJobsFilter) {
    let mut sep = "";
    if let Some(status) = filter.status {
        out.push_str(sep);
        out.push_str("status=");
        out.push_str(match status {
            crate::types::JobStatus::Queued => "queued",
            crate::types::JobStatus::Running => "running",
            crate::types::JobStatus::Completed => "completed",
            crate::types::JobStatus::Failed => "failed",
        });
        sep = "&";
    }
    if let Some(cursor) = &filter.cursor {
        out.push_str(sep);
        out.push_str("cursor=");
        out.push_str(cursor);
        sep = "&";
    }
    if let Some(limit) = filter.limit {
        out.push_str(sep);
        out.push_str("limit=");
        out.push_str(&limit.to_string());
    }
}
