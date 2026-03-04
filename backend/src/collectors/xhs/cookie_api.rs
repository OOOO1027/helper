use std::collections::HashMap;

use serde_json::Value;

use crate::env_runtime::env_with_shell_fallback;
use crate::{BackendError, Result};

use super::XHS_BASE_URL;
use super::XhsCollector;

impl XhsCollector {
    pub(super) fn resolve_cookie(&self) -> Option<String> {
        if let Some(cookie) = std::env::var("XHS_COOKIE")
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            return Some(cookie);
        }
        if let Some(path) = std::env::var("XHS_COOKIE_FILE")
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            if let Ok(raw) = std::fs::read_to_string(path) {
                let value = raw.trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
        env_with_shell_fallback("XHS_COOKIE").filter(|v| !v.trim().is_empty())
    }

    pub(super) async fn fetch_with_cookie(
        &self,
        path: &str,
        params: &HashMap<String, String>,
        cookie: &str,
    ) -> Result<Value> {
        let client = reqwest::Client::builder()
            .user_agent(&self.user_agent)
            .build()
            .map_err(|e| BackendError::Internal(format!("xhs http client init failed: {e}")))?;
        let query_pairs: Vec<(&str, &str)> = params
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let response = client
            .get(format!("{XHS_BASE_URL}{path}"))
            .header("accept", "application/json, text/plain, */*")
            .header("cookie", cookie)
            .query(&query_pairs)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("xhs http request failed: {e}")))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| BackendError::Internal(format!("xhs response decode failed: {e}")))?;
        if !status.is_success() {
            return Err(BackendError::Validation(format!(
                "xhs request rejected: http_status={status}"
            )));
        }
        serde_json::from_str(&body).map_err(|e| {
            BackendError::Internal(format!("xhs response json decode failed: {e}"))
        })
    }
}
