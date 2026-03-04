use crate::env_runtime::env_with_shell_fallback;
use std::fs;

use super::normalize_text_output;

pub(super) async fn fetch_xhs_page_text(source_url: &str, timeout_ms: u64) -> Option<String> {
    let url = source_url.trim();
    if url.is_empty() || !url.contains("xiaohongshu.com") {
        return None;
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
        .ok()?;
    let mut req = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
        );
    if let Some(cookie) = resolve_xhs_cookie() {
        req = req.header(reqwest::header::COOKIE, cookie);
    }
    let response = req.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let html = response.text().await.ok()?;
    let mut parts = Vec::new();
    if let Some(title) = extract_html_title(&html) {
        parts.push(title);
    }
    if let Some(desc) = extract_meta_content(&html, "description") {
        parts.push(desc);
    }
    if let Some(og_desc) = extract_meta_property_content(&html, "og:description") {
        parts.push(og_desc);
    }
    let merged = parts.join("\n");
    let cleaned = normalize_text_output(&merged, 3_000);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn resolve_xhs_cookie() -> Option<String> {
    if let Some(cookie) = env_with_shell_fallback("XHS_COOKIE")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Some(cookie);
    }
    let path = env_with_shell_fallback("XHS_COOKIE_FILE")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())?;
    let content = fs::read_to_string(path).ok()?;
    let cookie = content
        .lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty() && !line.starts_with('#'))?
        .to_string();
    if cookie.is_empty() {
        None
    } else {
        Some(cookie)
    }
}

fn extract_html_title(html: &str) -> Option<String> {
    let start = html.find("<title>")?;
    let end = html[start..].find("</title>")?;
    let content = &html[start + "<title>".len()..start + end];
    let text = normalize_text_output(content, 200);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn extract_meta_content(html: &str, name: &str) -> Option<String> {
    let marker = format!("name=\"{name}\"");
    let idx = html.find(&marker)?;
    extract_content_attr(&html[idx..], 512)
}

fn extract_meta_property_content(html: &str, property: &str) -> Option<String> {
    let marker = format!("property=\"{property}\"");
    let idx = html.find(&marker)?;
    extract_content_attr(&html[idx..], 512)
}

fn extract_content_attr(slice: &str, window: usize) -> Option<String> {
    let end = slice.len().min(window);
    let sample = &slice[..end];
    if let Some(start) = sample.find("content=\"") {
        let rest = &sample[start + 9..];
        let close = rest.find('"')?;
        let text = normalize_text_output(&rest[..close], 600);
        if !text.is_empty() {
            return Some(text);
        }
    }
    if let Some(start) = sample.find("content='") {
        let rest = &sample[start + 9..];
        let close = rest.find('\'')?;
        let text = normalize_text_output(&rest[..close], 600);
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}
