use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::pipeline::queue::{next_retry_delay_ms, should_route_failed_queue};
use crate::pipeline::score::{
    gate_bucket, quality_score, review_priority, should_trigger_review, value_score, GateBucket,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeInput {
    pub c: f64,
    pub s: f64,
    pub j: f64,
    pub d: f64,
    pub r: f64,
    pub f: f64,
    pub e: f64,
    pub p: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeOutput {
    pub quality_score: f64,
    pub value_score: f64,
    pub requires_review: bool,
    pub gate: GateBucketDto,
    pub priority: f64,
    pub labels: Vec<String>,
    pub summary: String,
    pub key_points: Vec<String>,
    pub tags: Vec<String>,
    pub final_category: String,
    pub route_reason: String,
    pub classification_confidence: f64,
    pub summary_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GateBucketDto {
    Direct,
    Review,
    Exception,
}

impl From<GateBucket> for GateBucketDto {
    fn from(value: GateBucket) -> Self {
        match value {
            GateBucket::Direct => GateBucketDto::Direct,
            GateBucket::Review => GateBucketDto::Review,
            GateBucket::Exception => GateBucketDto::Exception,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentAnalysisInput {
    pub metrics: AnalyzeInput,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisFailure {
    pub error_code: String,
    pub retryable: bool,
    pub next_retry_ms: Option<u64>,
    pub to_failed_queue: bool,
}

pub fn analyze(input: &ContentAnalysisInput) -> AnalyzeOutput {
    let metrics = &input.metrics;
    let labels = infer_labels(&input.text);
    let tags = normalize_tags(&labels);
    let summary = summarize(&input.text, 140);
    let key_points = extract_key_points(&input.text, 3);
    let (final_category, route_reason) = infer_final_category(&input.text, &tags);
    let class_confidence = classification_confidence(metrics.c, metrics.s);
    let summary_confidence = summary_confidence(metrics.s, summary.len());

    let q = quality_score(metrics.c, metrics.s, metrics.j, metrics.d);
    let v = value_score(metrics.r, metrics.f, metrics.e, metrics.p);
    let requires_review = should_trigger_review(metrics.c, v);
    let gate = if requires_review {
        GateBucket::Review
    } else {
        gate_bucket(q)
    };

    AnalyzeOutput {
        quality_score: q,
        value_score: v,
        requires_review,
        gate: gate.into(),
        priority: review_priority(metrics.c, v),
        labels: tags.clone(),
        summary,
        key_points,
        tags,
        final_category,
        route_reason,
        classification_confidence: class_confidence,
        summary_confidence,
    }
}

pub fn classify_failure(error_code: &str, attempt: u32) -> AnalysisFailure {
    let retryable = matches!(error_code, "MOD-3001" | "MOD-3002" | "MOD-3003");
    let next_retry_ms = if retryable {
        next_retry_delay_ms(attempt)
    } else {
        None
    };
    let to_failed_queue = should_route_failed_queue(attempt, retryable);
    AnalysisFailure {
        error_code: error_code.to_string(),
        retryable,
        next_retry_ms,
        to_failed_queue,
    }
}

fn infer_labels(text: &str) -> Vec<String> {
    let low = text.to_lowercase();
    let mut labels = Vec::new();
    if low.contains("notion") || low.contains("效率") || low.contains("工作流") {
        labels.push("效率工具".to_string());
    }
    if low.contains("rust") || low.contains("tauri") || low.contains("sql") {
        labels.push("后端工程".to_string());
    }
    if low.contains("微信") || low.contains("小红书") || low.contains("b站") {
        labels.push("内容采集".to_string());
    }
    if labels.is_empty() {
        labels.push("未分类".to_string());
    }
    labels
}

fn normalize_tags(labels: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for label in labels {
        let key = label.trim().to_lowercase();
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        out.push(label.trim().to_string());
    }
    if out.is_empty() {
        out.push("未分类".to_string());
    }
    if out.len() > 6 {
        out.truncate(6);
    }
    out
}

fn extract_key_points(text: &str, max_points: usize) -> Vec<String> {
    let mut points = Vec::new();
    for chunk in text.split(['\n', '。', '！', '!', '?', '？']) {
        let cleaned = chunk.split_whitespace().collect::<Vec<_>>().join(" ");
        let trimmed = cleaned.trim();
        if trimmed.len() < 8 {
            continue;
        }
        points.push(trimmed.chars().take(80).collect::<String>());
        if points.len() >= max_points {
            break;
        }
    }
    if points.is_empty() {
        points.push(summarize(text, 80));
    }
    points
}

fn infer_final_category(text: &str, tags: &[String]) -> (String, String) {
    let low = text.to_lowercase();
    if low.contains("学习")
        || low.contains("读书")
        || low.contains("课程")
        || low.contains("知识")
        || low.contains("rust")
        || low.contains("tauri")
    {
        return ("学习".to_string(), "keyword_match".to_string());
    }
    if low.contains("工作")
        || low.contains("效率")
        || low.contains("notion")
        || low.contains("会议")
        || low.contains("项目")
    {
        return ("工作".to_string(), "keyword_match".to_string());
    }
    if low.contains("健康") || low.contains("运动") || low.contains("睡眠") || low.contains("减脂")
    {
        return ("健康".to_string(), "keyword_match".to_string());
    }
    if low.contains("理财")
        || low.contains("基金")
        || low.contains("股票")
        || low.contains("预算")
        || low.contains("财务")
    {
        return ("财务".to_string(), "keyword_match".to_string());
    }
    if low.contains("灵感") || low.contains("创意") || low.contains("想法") {
        return ("灵感".to_string(), "keyword_match".to_string());
    }
    if low.contains("生活") || low.contains("家居") || low.contains("旅行") || low.contains("美食")
    {
        return ("生活".to_string(), "keyword_match".to_string());
    }
    if let Some(first_tag) = tags.first() {
        if first_tag != "未分类" {
            return ("Inbox".to_string(), "fallback_tag_unmapped".to_string());
        }
    }
    ("Inbox".to_string(), "fallback_default".to_string())
}

fn summarize(text: &str, max_chars: usize) -> String {
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() <= max_chars {
        return cleaned;
    }
    cleaned.chars().take(max_chars).collect::<String>()
}

fn classification_confidence(c: f64, s: f64) -> f64 {
    (0.7 * c.clamp(0.0, 1.0) + 0.3 * s.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

fn summary_confidence(s: f64, summary_len: usize) -> f64 {
    let len_factor = if summary_len >= 80 {
        1.0
    } else {
        summary_len as f64 / 80.0
    };
    (0.8 * s.clamp(0.0, 1.0) + 0.2 * len_factor).clamp(0.0, 1.0)
}

#[allow(dead_code)]
pub fn analyze_metrics_only(input: &AnalyzeInput) -> AnalyzeOutput {
    let q = quality_score(input.c, input.s, input.j, input.d);
    let v = value_score(input.r, input.f, input.e, input.p);
    let requires_review = should_trigger_review(input.c, v);
    let gate = if requires_review {
        GateBucket::Review
    } else {
        gate_bucket(q)
    };

    AnalyzeOutput {
        quality_score: q,
        value_score: v,
        requires_review,
        gate: gate.into(),
        priority: review_priority(input.c, v),
        labels: vec!["未分类".to_string()],
        summary: String::new(),
        key_points: vec!["暂无要点".to_string()],
        tags: vec!["未分类".to_string()],
        final_category: "Inbox".to_string(),
        route_reason: "metrics_only".to_string(),
        classification_confidence: input.c.clamp(0.0, 1.0),
        summary_confidence: input.s.clamp(0.0, 1.0),
    }
}
