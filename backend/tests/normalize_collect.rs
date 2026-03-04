use helper_backend::pipeline::normalize::{normalize, NormalizeInput};

#[test]
fn normalize_extracts_url_and_builds_dedupe_key() {
    let out = normalize(&NormalizeInput {
        source: "wechat".to_string(),
        source_url: "".to_string(),
        published_at: Some("2026-02-25T21:30:45+08:00".to_string()),
        collected_at: "2026-02-25 22:00:00".to_string(),
        content_raw: "看这个链接 https://example.com/path?a=1 很有用".to_string(),
    });

    assert_eq!(out.source, "wechat");
    assert_eq!(out.canonical_url, "https://example.com/path");
    assert_eq!(out.extracted_urls.len(), 1);
    assert!(!out.url_hash.is_empty());
    assert!(!out.dedupe_key.is_empty());
    assert_eq!(out.published_at.as_deref(), Some("2026-02-25 21:30:45"));
}
