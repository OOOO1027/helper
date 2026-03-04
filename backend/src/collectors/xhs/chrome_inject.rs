use std::collections::HashMap;

use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;

use crate::{BackendError, Result};

use super::utils::{escape_for_applescript, looks_like_html, run_osascript, strip_to_json_start};
use super::XhsCollector;

#[derive(Debug, Deserialize)]
struct BrowserFetchEnvelope {
    ok: bool,
    #[allow(dead_code)]
    status: Option<u16>,
    body: Option<String>,
    error: Option<String>,
}

impl XhsCollector {
    pub(super) async fn fetch_with_chrome(
        &self,
        path: &str,
        params: &HashMap<String, String>,
    ) -> Result<Value> {
        let params_json = serde_json::to_string(params)
            .map_err(|e| BackendError::Internal(format!("xhs params encode failed: {e}")))?;
        let path_json = serde_json::to_string(path)
            .map_err(|e| BackendError::Internal(format!("xhs path encode failed: {e}")))?;
        let result_key = format!("__helper_xhs_fetch_{}", Utc::now().timestamp_millis());
        let key_json = serde_json::to_string(&result_key)
            .map_err(|e| BackendError::Internal(format!("xhs key encode failed: {e}")))?;
        let start_js = format!(
            r#"(() => {{
  const key = {key_json};
  const path = {path_json};
  const params = {params_json};
  const url = new URL(path, "https://www.xiaohongshu.com");
  Object.entries(params).forEach(([k, v]) => {{
    if (v !== null && String(v).length > 0) {{
      url.searchParams.set(k, String(v));
    }}
  }});
  window[key] = "__PENDING__";
  Promise.resolve().then(async () => {{
    try {{
      const response = await fetch(url.toString(), {{
        credentials: "include",
        headers: {{ "accept": "application/json, text/plain, */*" }}
      }});
      const body = await response.text();
      window[key] = JSON.stringify({{ ok: true, status: response.status, body }});
    }} catch (error) {{
      window[key] = JSON.stringify({{ ok: false, error: String(error) }});
    }}
  }});
  return "started";
}})()"#
        );
        let poll_js = format!(
            r#"(() => {{
  const key = {key_json};
  return window[key] || "";
}})()"#
        );
        let cleanup_js = format!(
            r#"(() => {{
  const key = {key_json};
  try {{ delete window[key]; }} catch (_) {{}}
  return "";
}})()"#
        );
        let script = format!(
            r#"tell application "Google Chrome"
  if it is not running then
    launch
  end if
  if (count of windows) = 0 then
    make new window
  end if
  if (count of tabs of front window) = 0 then
    make new tab at end of tabs of front window with properties {{URL:"https://www.xiaohongshu.com/explore"}}
  end if
  set targetTab to active tab of front window
  if URL of targetTab does not contain "xiaohongshu.com" then
    set URL of targetTab to "https://www.xiaohongshu.com/explore"
    delay 2
  end if
  execute targetTab javascript "{start_js_escaped}"
  set pollResult to ""
  repeat 80 times
    delay 0.15
    set pollResult to execute targetTab javascript "{poll_js_escaped}"
    if pollResult is not "" and pollResult is not "__PENDING__" then
      exit repeat
    end if
  end repeat
  execute targetTab javascript "{cleanup_js_escaped}"
  return pollResult
end tell"#,
            start_js_escaped = escape_for_applescript(&start_js),
            poll_js_escaped = escape_for_applescript(&poll_js),
            cleanup_js_escaped = escape_for_applescript(&cleanup_js),
        );
        let output = run_osascript(&script)?;
        let trimmed = output.trim();
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("undefined")
            || trimmed == "null"
            || trimmed == "__PENDING__"
        {
            return Err(BackendError::Validation(
                "xhs browser fetch returned empty payload: enable Chrome menu `View -> Developer -> Allow JavaScript from Apple Events`, or configure XHS_COOKIE/XHS_COOKIE_FILE to run without AppleScript".to_string(),
            ));
        }
        let envelope: BrowserFetchEnvelope = serde_json::from_str(trimmed).map_err(|e| {
            let preview: String = trimmed.chars().take(120).collect();
            BackendError::Internal(format!(
                "xhs browser envelope decode failed: {e}; output_preview={preview}"
            ))
        })?;
        if !envelope.ok {
            return Err(BackendError::Validation(format!(
                "xhs browser fetch failed: {}",
                envelope.error.unwrap_or_else(|| "unknown".to_string())
            )));
        }
        let body = envelope
            .body
            .ok_or_else(|| BackendError::Internal("xhs browser fetch body missing".to_string()))?;
        let status = envelope.status.unwrap_or(200);
        let body_trimmed = body.trim();
        let body_preview: String = body_trimmed.chars().take(120).collect();
        if status < 200 || status >= 300 {
            return Err(BackendError::Validation(format!(
                "xhs browser request rejected: http_status={status}, body_preview={body_preview}"
            )));
        }
        if body_trimmed.is_empty() {
            return Err(BackendError::Validation(
                "xhs browser response body is empty: ensure Chrome is logged in to xiaohongshu.com and retry".to_string(),
            ));
        }
        if looks_like_html(body_trimmed) {
            return Err(BackendError::Validation(format!(
                "xhs browser returned html page (likely login/captcha): body_preview={body_preview}"
            )));
        }
        let json_payload = strip_to_json_start(body_trimmed);
        serde_json::from_str(json_payload).map_err(|e| {
            BackendError::Validation(format!(
                "xhs browser response is not valid json: {e}; body_preview={body_preview}"
            ))
        })
    }
}
