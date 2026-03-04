use std::collections::HashSet;

use chrono::{TimeZone, Utc};
use serde_json::{Map, Value};

use crate::collectors::{CollectWindow, CollectedItem};
use crate::{BackendError, Result};

// ── Env helpers ────────────────────────────────────────────────────────────────

pub(super) fn read_env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default)
}

pub(super) fn read_env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

// ── AppleScript helpers ────────────────────────────────────────────────────────

pub(super) fn run_osascript(script: &str) -> Result<String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("osascript")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| BackendError::Internal(format!("launch osascript failed: {e}")))?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| BackendError::Internal(format!("write osascript stdin failed: {e}")))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| BackendError::Internal(format!("wait osascript failed: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        return Ok(stdout);
    }
    let merged = if stderr.is_empty() {
        stdout.clone()
    } else if stdout.is_empty() {
        stderr.clone()
    } else {
        format!("{stdout} | {stderr}")
    };
    if merged.contains("AppleScript 执行 JavaScript 的功能已关闭")
        || merged.contains("JavaScript from Apple Events")
    {
        return Err(BackendError::Validation(
            "xhs collector blocked: enable Chrome menu `View -> Developer -> Allow JavaScript from Apple Events`, or configure XHS_COOKIE/XHS_COOKIE_FILE to run without AppleScript".to_string(),
        ));
    }
    Err(BackendError::Internal(format!(
        "xhs osascript failed: {merged}"
    )))
}

pub(super) fn escape_for_applescript(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

// ── Payload validation ────────────────────────────────────────────────────────

pub(super) fn ensure_payload_ok(payload: &Value) -> Result<()> {
    let code = payload.get("code").and_then(Value::as_i64).unwrap_or(0);
    let success = payload
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if code != 0 || !success {
        return Err(BackendError::Validation(format!(
            "xhs request rejected: code={code}, success={success}"
        )));
    }
    Ok(())
}

pub(super) fn looks_like_html(text: &str) -> bool {
    let head = text.trim_start();
    let lower = head.to_ascii_lowercase();
    lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
        || lower.starts_with("<body")
        || lower.contains("<head")
}

pub(super) fn strip_to_json_start(text: &str) -> &str {
    if text.starts_with(")]}',") {
        return text.trim_start_matches(")]}',").trim_start();
    }
    if let Some(idx) = text.find(|c| c == '{' || c == '[') {
        return &text[idx..];
    }
    text
}

// ── Fallback decisions ────────────────────────────────────────────────────────

pub(super) fn should_fallback_to_chrome(err: &BackendError) -> bool {
    match err {
        BackendError::Validation(msg) => {
            msg.contains("http_status=500")
                || msg.contains("http_status=401")
                || msg.contains("http_status=403")
        }
        BackendError::Internal(msg) => msg.contains("xhs response json decode failed"),
        _ => false,
    }
}

pub(super) fn should_fallback_to_profile_scrape(err: &BackendError) -> bool {
    match err {
        BackendError::Validation(msg) => {
            msg.contains("http_status=500")
                || msg.contains("create invoker failed")
                || msg.contains("xhs browser request rejected")
                || msg.contains("xhs request rejected: http_status=500")
                || msg.contains("xhs browser response is not valid json")
        }
        BackendError::Internal(msg) => {
            msg.contains("xhs browser envelope decode failed")
                || msg.contains("xhs browser body decode failed")
                || msg.contains("xhs response json decode failed")
        }
        _ => false,
    }
}

// ── Text normalisation ────────────────────────────────────────────────────────

pub(super) fn normalize_content_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub(super) fn normalize_profile_item_url(url: &str, id: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return format!("https://www.xiaohongshu.com/explore/{id}");
    }
    trimmed
        .split('#')
        .next()
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("https://www.xiaohongshu.com/explore/{id}"))
}

pub(super) fn truncate_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

// ── Page / item extraction ────────────────────────────────────────────────────

pub(super) fn extract_page(payload: &Value) -> super::PageExtract {
    let mut extracted = super::PageExtract::default();
    let data = payload.get("data").unwrap_or(payload);
    let mut candidates = Vec::new();
    collect_note_candidates(data, &mut candidates);
    if candidates.is_empty() && !std::ptr::eq(data, payload) {
        collect_note_candidates(payload, &mut candidates);
    }
    for candidate in candidates {
        if let Some(item) = to_collected_item(&candidate) {
            extracted.items.push(item);
        }
    }
    extracted.has_more = first_bool(data, &["has_more", "more"]);
    extracted.next_cursor = first_string(data, &["next_cursor", "cursor"]).and_then(|v| {
        if v.trim().is_empty() {
            None
        } else {
            Some(v)
        }
    });
    extracted
}

fn collect_note_candidates(value: &Value, out: &mut Vec<Value>) {
    match value {
        Value::Object(map) => {
            if looks_like_note(map) {
                out.push(value.clone());
            }
            for child in map.values() {
                collect_note_candidates(child, out);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect_note_candidates(child, out);
            }
        }
        _ => {}
    }
}

fn looks_like_note(map: &Map<String, Value>) -> bool {
    let has_id = map.get("note_id").and_then(Value::as_str).is_some()
        || map.get("id").and_then(Value::as_str).is_some();
    if !has_id {
        return false;
    }
    map.contains_key("xsec_token")
        || map.contains_key("note_card")
        || map.contains_key("desc")
        || map.contains_key("title")
        || map.contains_key("display_title")
}

fn to_collected_item(value: &Value) -> Option<CollectedItem> {
    let external_id = first_string(value, &["note_id", "id"])?;
    if external_id.trim().is_empty() {
        return None;
    }
    let title = first_string(
        value,
        &[
            "title",
            "display_title",
            "note_card.title",
            "note_card.display_title",
        ],
    );
    let desc = first_string(
        value,
        &[
            "desc",
            "content",
            "note_card.desc",
            "note_card.content",
            "video_info.title",
        ],
    );
    let content_raw = merge_content(title.as_deref(), desc.as_deref())?;
    let token = first_string(value, &["xsec_token", "note_card.xsec_token"]);
    let direct_url = first_string(value, &["source_url", "note_url", "url", "share_url"]);
    let source_url = if let Some(url) = direct_url {
        url
    } else if let Some(xsec_token) = token {
        format!("https://www.xiaohongshu.com/explore/{external_id}?xsec_token={xsec_token}")
    } else {
        format!("https://www.xiaohongshu.com/explore/{external_id}")
    };
    let published_at = first_string(
        value,
        &[
            "published_at",
            "publish_time",
            "time",
            "create_time",
            "update_time",
        ],
    )
    .and_then(|v| normalize_ts(&v));
    let mut image_urls = extract_image_urls(value);
    let cover_url = first_string(
        value,
        &[
            "cover.url_default",
            "cover.url",
            "note_card.cover.url_default",
            "note_card.cover.url",
            "image_info.url_default",
            "image_info.url",
            "video_info.image.url_default",
            "video_info.image.url",
            "note_card.video_info.image.url_default",
            "note_card.video_info.image.url",
        ],
    )
    .or_else(|| image_urls.first().cloned());
    if let Some(ref cover) = cover_url {
        if !image_urls.iter().any(|v| v == cover) {
            image_urls.insert(0, cover.clone());
        }
    }
    if image_urls.len() > 8 {
        image_urls.truncate(8);
    }
    Some(CollectedItem {
        external_id,
        source_url,
        title,
        content_raw,
        published_at,
        cover_url,
        image_urls,
    })
}

fn extract_image_urls(root: &Value) -> Vec<String> {
    let mut urls = Vec::new();
    for path in [
        "image_list",
        "images_list",
        "images",
        "note_card.image_list",
        "note_card.images_list",
        "note_card.images",
        "image_info_list",
        "note_card.image_info_list",
        "note_card.image_info.image_list",
        "note_card.note_card.image_list",
    ] {
        if let Some(value) = get_path(root, path) {
            collect_image_urls(value, &mut urls);
        }
    }
    if urls.is_empty() {
        collect_image_urls(root, &mut urls);
    }
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for url in urls {
        if seen.insert(url.clone()) {
            deduped.push(url);
        }
    }
    deduped
}

fn collect_image_urls(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(v) => {
            let trimmed = v.trim();
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                out.push(trimmed.to_string());
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_image_urls(item, out);
            }
        }
        Value::Object(map) => {
            for key in [
                "url_default",
                "url",
                "url_pre",
                "image_url",
                "src",
                "origin",
                "master_url",
            ] {
                if let Some(Value::String(v)) = map.get(key) {
                    let trimmed = v.trim();
                    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                        out.push(trimmed.to_string());
                    }
                }
            }
            for nested in map.values() {
                collect_image_urls(nested, out);
            }
        }
        _ => {}
    }
}

fn merge_content(title: Option<&str>, desc: Option<&str>) -> Option<String> {
    match (title, desc) {
        (Some(t), Some(d))
            if !t.trim().is_empty() && !d.trim().is_empty() && t.trim() != d.trim() =>
        {
            Some(format!("{}\n{}", t.trim(), d.trim()))
        }
        (Some(t), _) if !t.trim().is_empty() => Some(t.trim().to_string()),
        (_, Some(d)) if !d.trim().is_empty() => Some(d.trim().to_string()),
        _ => None,
    }
}

fn normalize_ts(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(mut ts) = trimmed.parse::<i64>() {
        if ts > 10_000_000_000 {
            ts /= 1000;
        }
        let dt = Utc.timestamp_opt(ts, 0).single()?;
        return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    Some(trimmed.to_string())
}

pub(super) fn first_string(root: &Value, paths: &[&str]) -> Option<String> {
    for path in paths {
        if let Some(value) = get_path(root, path) {
            match value {
                Value::String(v) if !v.trim().is_empty() => return Some(v.trim().to_string()),
                Value::Number(v) => return Some(v.to_string()),
                _ => {}
            }
        }
    }
    None
}

pub(super) fn first_bool(root: &Value, paths: &[&str]) -> Option<bool> {
    for path in paths {
        if let Some(value) = get_path(root, path) {
            if let Some(v) = value.as_bool() {
                return Some(v);
            }
            if let Some(v) = value.as_i64() {
                return Some(v != 0);
            }
        }
    }
    None
}

fn get_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for seg in path.split('.') {
        match current {
            Value::Object(map) => current = map.get(seg)?,
            _ => return None,
        }
    }
    Some(current)
}

pub(super) fn within_window(window: &CollectWindow, published_at: Option<&str>) -> bool {
    let Some(ts) = published_at else {
        return true;
    };
    let since_ok = window
        .since
        .as_deref()
        .map(|since| ts >= since)
        .unwrap_or(true);
    let until_ok = window
        .until
        .as_deref()
        .map(|until| ts <= until)
        .unwrap_or(true);
    since_ok && until_ok
}

pub(super) fn stable_hash(input: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in input.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
