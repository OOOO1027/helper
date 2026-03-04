use chrono::Local;
use rusqlite::{params, Connection};
use serde_json::{json, Value};

use super::quality_eval::evaluate_structured_quality;
use super::xhs_scraper::fetch_xhs_page_text;
use super::{
    dedupe_string_vec, extract_string_list, normalize_string_list, normalize_text_output,
    BudgetGuardPolicy, ModelUsageRecord, PendingSyncRow, StructuredContent,
    StructuredContentConfig, StructuredContentState,
};
use crate::config;
use crate::env_runtime::env_with_shell_fallback;
use crate::{BackendError, Result};

pub(super) fn load_structured_content_config() -> StructuredContentConfig {
    let enabled = config::env_bool("B2_G2_AI_ENABLED", true);
    let api_key = env_with_shell_fallback("QWEN_API_KEY")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let model = env_with_shell_fallback("QWEN_MODEL")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "qwen-flash".to_string());
    let max_calls_per_run = config::env_usize("B2_G2_QWEN_MAX_CALLS_PER_RUN", 20).max(1);
    let reduced_max_calls_per_run = config::env_usize(
        "B2_G2_QWEN_MAX_CALLS_REDUCED",
        ((max_calls_per_run as f64) * 0.5).ceil() as usize,
    )
    .max(1)
    .min(max_calls_per_run);
    let budget_limit_cny = config::env_f64("B2_G2_MONTHLY_BUDGET_CNY", 100.0).max(1.0);
    let budget_degrade_ratio = config::env_f64("B2_G2_BUDGET_DEGRADE_RATIO", 0.80).clamp(0.0, 1.0);
    let budget_fuse_ratio = config::env_f64("B2_G2_BUDGET_FUSE_RATIO", 1.0)
        .max(budget_degrade_ratio)
        .clamp(0.0, 5.0);
    let deep_fetch_enabled = config::env_bool("B2_G2_XHS_DEEP_FETCH", true);
    let fetch_timeout_ms = config::env_u64("B2_G2_XHS_DEEP_FETCH_TIMEOUT_MS", 8_000).max(1_000);
    StructuredContentConfig {
        enabled,
        api_key,
        model,
        max_calls_per_run,
        reduced_max_calls_per_run,
        budget_limit_cny,
        budget_degrade_ratio,
        budget_fuse_ratio,
        deep_fetch_enabled,
        fetch_timeout_ms,
    }
}

pub(super) fn resolve_budget_guard_policy(
    conn: &Connection,
    cfg: &StructuredContentConfig,
) -> Result<BudgetGuardPolicy> {
    let month_key = Local::now().format("%Y-%m").to_string();
    let used_cny: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(cost_cny), 0.0)
             FROM budget_ledger
             WHERE day LIKE (?1 || '%')",
            params![month_key],
            |row| row.get(0),
        )
        .unwrap_or(0.0);
    let usage_ratio = if cfg.budget_limit_cny <= 0.0 {
        0.0
    } else {
        (used_cny.max(0.0) / cfg.budget_limit_cny).max(0.0)
    };
    let fuse_active = usage_ratio >= cfg.budget_fuse_ratio;
    let reduce_active = !fuse_active && usage_ratio >= cfg.budget_degrade_ratio;
    let model_call_cap = if fuse_active {
        0
    } else if reduce_active {
        cfg.reduced_max_calls_per_run
    } else {
        cfg.max_calls_per_run
    };
    Ok(BudgetGuardPolicy {
        usage_ratio,
        model_call_cap,
        fuse_active,
        reduce_active,
    })
}

pub(super) fn refresh_budget_guard_state(
    conn: &Connection,
    state: &mut StructuredContentState,
) -> Result<()> {
    let policy = resolve_budget_guard_policy(conn, &state.config)?;
    state.budget_usage_ratio = policy.usage_ratio;
    state.model_call_cap = policy.model_call_cap;
    state.fuse_active = policy.fuse_active;
    state.reduce_active = policy.reduce_active;
    Ok(())
}

pub(super) fn model_skip_reason(state: &StructuredContentState) -> Option<&'static str> {
    if !state.config.enabled {
        return Some("model_disabled");
    }
    if state.fuse_active {
        return Some("budget_fused");
    }
    if state.config.api_key.is_none() {
        return Some("model_api_key_missing");
    }
    if state.calls_used >= state.model_call_cap {
        if state.reduce_active {
            return Some("budget_reduced_cap");
        }
        return Some("model_call_cap_reached");
    }
    None
}

pub(super) async fn build_structured_content(
    row: &PendingSyncRow,
    route_reason: Option<&str>,
    state: &mut StructuredContentState,
) -> Result<StructuredContent> {
    let mut summary = normalize_text_output(&row.summary, 5_000);
    let mut key_points = normalize_string_list(&row.key_points, 6, 200);
    let mut tags = normalize_string_list(&row.tags, 8, 50);
    if tags.is_empty() {
        tags = normalize_string_list(&row.labels, 8, 50);
    }
    let mut degraded_fields = Vec::<String>::new();
    let mut content_source = "rules".to_string();
    let mut usage: Option<ModelUsageRecord> = None;

    let skip_reason = model_skip_reason(state);
    let can_call_model = skip_reason.is_none();
    if can_call_model {
        state.calls_used += 1;
        let source_text = build_ai_source_text(row, &state.config).await;
        match generate_structured_with_qwen(
            state.config.api_key.as_deref().unwrap_or_default(),
            &state.config.model,
            route_reason,
            &source_text,
        )
        .await
        {
            Ok(Some(model_output)) => {
                content_source = "qwen".to_string();
                usage = model_output.usage.clone();
                let model_summary = normalize_text_output(&model_output.summary, 5_000);
                if !model_summary.is_empty() {
                    summary = model_summary;
                } else {
                    degraded_fields.push("summary".to_string());
                }
                let model_points = normalize_string_list(&model_output.key_points, 6, 200);
                if !model_points.is_empty() {
                    key_points = model_points;
                } else {
                    degraded_fields.push("key_points".to_string());
                }
                let model_tags = normalize_string_list(&model_output.tags, 8, 50);
                if !model_tags.is_empty() {
                    tags = model_tags;
                } else {
                    degraded_fields.push("tags".to_string());
                }
                for field in model_output.degraded_fields {
                    if !field.trim().is_empty() {
                        degraded_fields.push(field.trim().to_string());
                    }
                }
            }
            Ok(None) => degraded_fields.push("model_empty".to_string()),
            Err(_) => degraded_fields.push("model_unavailable".to_string()),
        }
    } else if let Some(reason) = skip_reason {
        degraded_fields.push(reason.to_string());
    }

    if summary.trim().is_empty() {
        summary = "-".to_string();
    }
    if key_points.is_empty() {
        key_points = normalize_string_list(&row.key_points, 6, 200);
    }
    if tags.is_empty() {
        tags = normalize_string_list(&row.labels, 8, 50);
    }
    if tags.is_empty() {
        tags.push("未分类".to_string());
    }
    if row.cover_url.as_deref().unwrap_or("").trim().is_empty() && row.image_urls.is_empty() {
        degraded_fields.push("images".to_string());
    }

    let (mut quality_score, mut quality_state, mut quality_degraded) =
        evaluate_structured_quality(row, &summary, &key_points, &tags, route_reason);
    degraded_fields.append(&mut quality_degraded);
    if degraded_fields
        .iter()
        .any(|field| field == "model_api_key_missing")
    {
        quality_score = quality_score.min(40.0);
        quality_state = "failed".to_string();
    }
    dedupe_string_vec(&mut degraded_fields);
    if quality_state == "success" && !degraded_fields.is_empty() {
        quality_state = "degraded".to_string();
    }

    Ok(StructuredContent {
        summary,
        key_points,
        tags,
        quality_score,
        quality_state,
        degraded_fields,
        content_source,
        usage,
    })
}

#[derive(Debug, Clone, Default)]
struct QwenStructuredOutput {
    summary: String,
    key_points: Vec<String>,
    tags: Vec<String>,
    degraded_fields: Vec<String>,
    usage: Option<ModelUsageRecord>,
}

async fn build_ai_source_text(row: &PendingSyncRow, config: &StructuredContentConfig) -> String {
    let mut text = row.content_raw.clone();
    if text.trim().is_empty() || text.trim() == "-" {
        text = row.summary.clone();
    }
    if text.trim().is_empty() || text.trim() == "-" {
        text = row.title.clone();
    }
    if config.deep_fetch_enabled && row.source == "xhs" {
        if let Some(source_url) = row.source_url.as_deref() {
            if let Some(fetched_text) =
                fetch_xhs_page_text(source_url, config.fetch_timeout_ms).await
            {
                if !fetched_text.trim().is_empty() {
                    text = format!("{text}\n{fetched_text}");
                }
            }
        }
    }
    normalize_text_output(&text, 6_000)
}

async fn generate_structured_with_qwen(
    api_key: &str,
    model: &str,
    route_reason: Option<&str>,
    source_text: &str,
) -> Result<Option<QwenStructuredOutput>> {
    if api_key.trim().is_empty() || source_text.trim().is_empty() {
        return Ok(None);
    }
    let prompt = format!(
        "你是知识提炼引擎。请只输出 JSON，对应字段：summary,key_points,tags,degraded_fields。\
summary 要中文 80-260 字，强调结论与可执行动作；\
key_points 输出 3-6 条，每条 <= 60 字；\
tags 输出 3-8 个，不要泛化标签。\
不得编造原文没有的事实。若信息不足，请把对应字段写入 degraded_fields。\
route_reason: {}。\
原文如下：\n{}",
        route_reason.unwrap_or("null"),
        source_text
    );
    let body = json!({
        "model": model,
        "temperature": 0.2,
        "max_tokens": 700,
        "enable_thinking": false,
        "response_format": { "type": "json_object" },
        "messages": [
            {
                "role": "system",
                "content": "你是严格的中文知识整理助手，只输出 JSON。"
            },
            {
                "role": "user",
                "content": prompt
            }
        ]
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(18))
        .build()
        .map_err(|e| BackendError::Internal(format!("qwen client init failed: {e}")))?;
    let resp = client
        .post("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| BackendError::Internal(format!("qwen request failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(BackendError::Internal(format!(
            "qwen request rejected: http_status={}",
            resp.status().as_u16()
        )));
    }
    let raw = resp
        .text()
        .await
        .map_err(|e| BackendError::Internal(format!("qwen response decode failed: {e}")))?;
    let envelope = serde_json::from_str::<Value>(&raw)
        .map_err(|e| BackendError::Internal(format!("qwen json decode failed: {e}")))?;
    let content = envelope
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if content.trim().is_empty() {
        return Ok(None);
    }
    let normalized = extract_json_object_text(content).unwrap_or_else(|| content.to_string());
    let payload = serde_json::from_str::<Value>(&normalized)
        .map_err(|e| BackendError::Internal(format!("qwen payload json decode failed: {e}")))?;
    let summary = payload
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    let key_points = extract_string_list(payload.get("key_points"));
    let tags = extract_string_list(payload.get("tags"));
    let degraded_fields = extract_string_list(payload.get("degraded_fields"));
    let usage = parse_qwen_usage(&envelope, model);
    Ok(Some(QwenStructuredOutput {
        summary,
        key_points,
        tags,
        degraded_fields,
        usage,
    }))
}

fn parse_qwen_usage(envelope: &Value, model: &str) -> Option<ModelUsageRecord> {
    let usage = envelope.get("usage")?;
    let tokens_in = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_u64()))
        .or_else(|| usage.get("promptTokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    let tokens_out = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_u64()))
        .or_else(|| usage.get("completionTokens").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    if tokens_in == 0 && tokens_out == 0 {
        return None;
    }
    Some(ModelUsageRecord {
        provider: "qwen".to_string(),
        model: model.to_string(),
        purpose: "b2_g2_structured_summary".to_string(),
        tokens_in,
        tokens_out,
        cost_cny: estimate_qwen_cost_cny(model, tokens_in, tokens_out),
    })
}

fn estimate_qwen_cost_cny(model: &str, tokens_in: u64, tokens_out: u64) -> f64 {
    let normalized = model.trim().to_ascii_lowercase();
    let (in_per_million, out_per_million) = if normalized.contains("qwen-plus") {
        (0.8_f64, 2.0_f64)
    } else if normalized.contains("qwen-flash") {
        (0.15_f64, 1.5_f64)
    } else {
        // qwen-turbo / qwen-turbo-latest fallback
        (0.3_f64, 0.6_f64)
    };
    ((tokens_in as f64 / 1_000_000.0) * in_per_million)
        + ((tokens_out as f64 / 1_000_000.0) * out_per_million)
}

fn extract_json_object_text(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(trimmed[start..=end].to_string())
}
