use super::{dedupe_string_vec, normalize_text_output, PendingSyncRow};

pub(super) fn evaluate_structured_quality(
    row: &PendingSyncRow,
    summary: &str,
    key_points: &[String],
    tags: &[String],
    route_reason: Option<&str>,
) -> (f64, String, Vec<String>) {
    let mut score = 0.0;
    let mut degraded = Vec::<String>::new();
    let source_text = normalize_text_output(&format!("{}\n{}", row.content_raw, row.summary), 6_000);
    let summary_clean = summary.trim();

    if !summary_clean.is_empty() && summary_clean != "-" {
        score += 10.0;
    } else {
        degraded.push("summary".to_string());
    }
    if key_points.len() >= 3 {
        score += 10.0;
    } else {
        score += (key_points.len() as f64 / 3.0) * 10.0;
        degraded.push("key_points".to_string());
    }
    let useful_tags = tags
        .iter()
        .filter(|tag| {
            let trimmed = tag.trim();
            !trimmed.is_empty() && trimmed != "未分类"
        })
        .count();
    if useful_tags >= 3 {
        score += 5.0;
    } else {
        score += (useful_tags as f64 / 3.0) * 5.0;
        degraded.push("tags".to_string());
    }

    let summary_overlap = overlap_ratio(summary_clean, &source_text);
    score += if summary_overlap >= 0.18 {
        15.0
    } else if summary_overlap >= 0.08 {
        8.0
    } else {
        degraded.push("factuality".to_string());
        2.0
    };
    let point_overlap = key_points
        .iter()
        .filter(|point| overlap_ratio(point, &source_text) >= 0.06)
        .count();
    score += if point_overlap >= 2 {
        10.0
    } else if point_overlap == 1 {
        5.0
    } else {
        degraded.push("evidence".to_string());
        1.0
    };

    let actionable_hits = action_keyword_hits(summary_clean, key_points);
    score += if actionable_hits >= 3 {
        20.0
    } else if actionable_hits >= 1 {
        12.0
    } else {
        degraded.push("actionability".to_string());
        4.0
    };

    let summary_len = summary_clean.chars().count();
    score += if (80..=260).contains(&summary_len) {
        10.0
    } else if (50..=360).contains(&summary_len) {
        6.0
    } else {
        degraded.push("summary_length".to_string());
        2.0
    };
    score += if (3..=6).contains(&key_points.len()) {
        10.0
    } else if !key_points.is_empty() {
        6.0
    } else {
        degraded.push("readability".to_string());
        1.0
    };

    score += 5.0;
    if !matches!(
        route_reason,
        None | Some("low_confidence" | "growth_limit_exceeded" | "keyword_match" | "fallback_default" | "fallback_tag_unmapped")
    ) {
        degraded.push("route_reason".to_string());
    }
    score += 5.0;

    dedupe_string_vec(&mut degraded);
    let mut clamped = score.clamp(0.0, 100.0);
    if clamped < 55.0
        && !summary_clean.is_empty()
        && source_text.chars().count() < 200
    {
        clamped = 60.0;
        degraded.push("short_source_context".to_string());
    }
    let state = if clamped >= 75.0 {
        "success"
    } else if clamped >= 55.0 {
        "degraded"
    } else {
        "failed"
    };
    (clamped, state.to_string(), degraded)
}

fn overlap_ratio(candidate: &str, source: &str) -> f64 {
    let c = candidate.trim();
    let s = source.trim();
    if c.is_empty() || s.is_empty() {
        return 0.0;
    }
    let sample = c.chars().take(80).collect::<String>();
    if sample.is_empty() {
        return 0.0;
    }
    let matched_chars = sample
        .chars()
        .filter(|ch| !ch.is_whitespace() && s.contains(*ch))
        .count();
    matched_chars as f64 / sample.chars().count() as f64
}

fn action_keyword_hits(summary: &str, key_points: &[String]) -> usize {
    let keywords = [
        "建议", "步骤", "执行", "行动", "策略", "框架", "先", "然后", "最后", "复盘",
    ];
    let mut text = summary.to_string();
    for point in key_points {
        text.push('\n');
        text.push_str(point);
    }
    keywords.iter().filter(|kw| text.contains(*kw)).count()
}
