//! LLM-settings endpoints on [`crate::Client`].
//!
//! Routes registered in `internal/server/routes.go` under
//! `setupV1Routes` when an [`LLMSettingsHandlers`] instance is wired.
//! The server stores settings on the per-org tenant; reads return the
//! defaults when no tenant exists yet so the UI can render a fresh form
//! without a hard 404.

use reqwest::Method;

use crate::client::Client;
use crate::error::Error;
use crate::types::LlmSettings;

impl Client {
    /// `GET /v1/orgs/{orgID}/llm-settings`. The `api_key` field is
    /// returned masked (`***...`).
    pub async fn get_llm_settings(&self, org_id: &str) -> Result<LlmSettings, Error> {
        let path = format!("v1/orgs/{}/llm-settings", org_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `PATCH /v1/orgs/{orgID}/llm-settings` — partial merge.
    ///
    /// Only the fields populated on `settings` overwrite the stored
    /// values; omitted fields keep their previous content. Echoing back
    /// the masked `***...` sentinel preserves the existing `api_key`.
    /// The response is the merged + masked settings object.
    pub async fn update_llm_settings(
        &self,
        org_id: &str,
        settings: LlmSettings,
    ) -> Result<LlmSettings, Error> {
        let path = format!("v1/orgs/{}/llm-settings", org_id);
        self.request_json(Method::PATCH, &path, Some(&settings)).await
    }

    /// `DELETE /v1/orgs/{orgID}/llm-settings`. Resets to defaults and
    /// returns the default settings as the response body.
    pub async fn delete_llm_settings(&self, org_id: &str) -> Result<LlmSettings, Error> {
        let path = format!("v1/orgs/{}/llm-settings", org_id);
        self.request_json::<(), LlmSettings>(Method::DELETE, &path, Option::<&()>::None)
            .await
    }
}
