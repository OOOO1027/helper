//! Pure Notion block-rendering helpers.  No database access, no async.
//! All functions take plain string / slice inputs and return formatted strings
//! or `serde_json::Value` blocks ready to be sent to the Notion API.

use serde_json::{json, Value};

pub(super) fn paragraph_block(text: &str) -> Value {
    json!({
        "object": "block",
        "type": "paragraph",
        "paragraph": {
            "rich_text": [{
                "type": "text",
                "text": { "content": text }
            }]
        }
    })
}

pub(super) fn render_summary_block(summary: &str) -> String {
    let cleaned = summary.trim();
    if cleaned.is_empty() {
        "结论摘要\n-".to_string()
    } else {
        format!("结论摘要\n{cleaned}")
    }
}

pub(super) fn render_key_points_block(key_points: &[String], summary: &str) -> String {
    let mut lines = Vec::new();
    for point in key_points.iter().filter(|v| !v.trim().is_empty()).take(5) {
        lines.push(format!("- {}", point.trim()));
    }
    if lines.is_empty() {
        let fallback = summary.trim();
        if fallback.is_empty() || fallback == "-" {
            lines.push("- 暂无要点".to_string());
        } else {
            lines.push(format!(
                "- {}",
                fallback.chars().take(80).collect::<String>()
            ));
        }
    }
    format!("关键要点\n{}", lines.join("\n"))
}

pub(super) fn render_source_link_block(source_url: Option<&str>) -> String {
    let url = source_url
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .unwrap_or("-");
    format!("原文链接\n{url}")
}

pub(super) fn render_tags_block(tags: &[String], labels: &[String]) -> String {
    let values = if tags.is_empty() { labels } else { tags };
    if values.is_empty() {
        return "标签\n未分类".to_string();
    }
    format!("标签\n{}", values.join(", "))
}

pub(super) fn render_images_block(cover_url: Option<&str>, image_urls: &[String]) -> String {
    let cover = cover_url
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let mut lines = Vec::new();
    if let Some(ref url) = cover {
        lines.push(format!("封面: {url}"));
    }
    for (idx, url) in image_urls
        .iter()
        .filter(|v| !v.trim().is_empty())
        .take(3)
        .enumerate()
    {
        lines.push(format!("图{}: {}", idx + 1, url.trim()));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("图片\n{}", lines.join("\n"))
    }
}

/// Normalises a node key to lowercase for stable tree lookups.
pub(super) fn normalize_node_key(raw: &str) -> String {
    raw.trim().to_lowercase()
}
