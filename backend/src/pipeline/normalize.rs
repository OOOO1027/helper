use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizeInput {
    pub source: String,
    pub source_url: String,
    pub published_at: Option<String>,
    pub collected_at: String,
    pub content_raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizeOutput {
    pub source: String,
    pub canonical_url: String,
    pub extracted_urls: Vec<String>,
    pub url_hash: String,
    pub text_clean: String,
    pub fingerprint: String,
    pub dedupe_key: String,
    pub published_at: Option<String>,
    pub collected_at: String,
}

pub fn normalize(input: &NormalizeInput) -> NormalizeOutput {
    let extracted_urls = extract_urls(&input.content_raw);
    let canonical_url = if input.source_url.trim().is_empty() {
        extracted_urls
            .first()
            .cloned()
            .unwrap_or_else(|| "about:blank".to_string())
    } else {
        canonicalize_url(&input.source_url)
    };
    let text_clean = input
        .content_raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let fingerprint = format!("{:x}", md5_like_hash(text_clean.as_bytes()));
    let url_hash = format!("{:x}", md5_like_hash(canonical_url.as_bytes()));
    let dedupe_key = build_dedupe_key(
        &input.source,
        &url_hash,
        &fingerprint,
        input.published_at.as_deref(),
        &input.collected_at,
    );

    NormalizeOutput {
        source: input.source.clone(),
        canonical_url,
        extracted_urls,
        url_hash,
        text_clean,
        fingerprint,
        dedupe_key,
        published_at: normalize_ts(input.published_at.as_deref()),
        collected_at: normalize_ts(Some(&input.collected_at))
            .unwrap_or_else(|| input.collected_at.clone()),
    }
}

pub fn canonicalize_url(url: &str) -> String {
    let lower = url.trim().to_lowercase();
    let mut parts = lower.split('?');
    let base = parts.next().unwrap_or_default().to_string();
    base
}

pub fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for raw in text.split_whitespace() {
        let candidate = raw
            .trim_matches(|c: char| ",.;:!?()[]{}\"'".contains(c))
            .to_string();
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            urls.push(canonicalize_url(&candidate));
        }
    }
    urls.sort();
    urls.dedup();
    urls
}

pub fn normalize_ts(ts: Option<&str>) -> Option<String> {
    let val = ts?.trim();
    if val.is_empty() {
        return None;
    }
    if val.len() >= 19 {
        return Some(val[..19].replace('T', " "));
    }
    Some(val.to_string())
}

pub fn build_dedupe_key(
    source: &str,
    url_hash: &str,
    fingerprint: &str,
    published_at: Option<&str>,
    collected_at: &str,
) -> String {
    let date_scope = published_at
        .and_then(|ts| normalize_ts(Some(ts)))
        .or_else(|| normalize_ts(Some(collected_at)))
        .unwrap_or_else(|| collected_at.to_string());
    let day = date_scope.chars().take(10).collect::<String>();
    let raw = format!("{source}|{url_hash}|{fingerprint}|{day}");
    format!("{:x}", md5_like_hash(raw.as_bytes()))
}

fn md5_like_hash(bytes: &[u8]) -> u64 {
    // 轻量占位哈希，后续可替换为稳定内容指纹实现。
    let mut h = 1469598103934665603u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
