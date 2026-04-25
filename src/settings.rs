//! LLM-settings endpoints on [`crate::Client`].
//!
//! These routes are documented on `internal/server/settings.go` but are
//! not currently registered in the default router; they will light up on
//! servers that wire the `SettingsHandlers` instance.

use reqwest::Method;

use crate::client::Client;
use crate::error::Error;
use crate::types::LlmSettings;

impl Client {
    /// `GET /v1/orgs/{orgID}/settings/llm`.
    pub async fn get_llm_settings(&self, org_id: &str) -> Result<LlmSettings, Error> {
        let path = format!("v1/orgs/{}/settings/llm", org_id);
        self.request_json(Method::GET, &path, Option::<&()>::None)
            .await
    }

    /// `PUT /v1/orgs/{orgID}/settings/llm`.
    pub async fn update_llm_settings(
        &self,
        org_id: &str,
        settings: LlmSettings,
    ) -> Result<serde_json::Value, Error> {
        let path = format!("v1/orgs/{}/settings/llm", org_id);
        self.request_json::<_, serde_json::Value>(Method::PUT, &path, Some(&settings))
            .await
    }

    /// `DELETE /v1/orgs/{orgID}/settings/llm`. Resets settings to defaults.
    pub async fn delete_llm_settings(&self, org_id: &str) -> Result<serde_json::Value, Error> {
        let path = format!("v1/orgs/{}/settings/llm", org_id);
        self.request_json::<(), serde_json::Value>(Method::DELETE, &path, Option::<&()>::None)
            .await
    }
}
