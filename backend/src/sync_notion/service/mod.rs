use chrono::{DateTime, Duration, Local, NaiveDateTime, NaiveDateTime as ChronoNaiveDateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

use crate::config;
use crate::sync_notion::client::{NotionClient, NotionRecord, RetryPolicy};
use crate::sync_notion::notion_api::NotionTreeClient;
use crate::sync_notion::tree::week_key_and_title;
use crate::{BackendError, Result};

mod render;

mod ai_content;
mod quality_eval;
mod state_persistence;
mod tree_builder;
mod xhs_scraper;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRunSummary {
    pub scanned: u32,
    pub attempted: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub requeued: u32,
    pub dead_lettered: u32,
}

#[derive(Debug, Clone)]
struct PendingSyncRow {
    sync_record_id: String,
    normalized_item_id: String,
    source: String,
    source_url: Option<String>,
    content_raw: String,
    published_at: Option<String>,
    cover_url: Option<String>,
    image_urls: Vec<String>,
    title: String,
    summary: String,
    labels: Vec<String>,
    tags: Vec<String>,
    key_points: Vec<String>,
    final_category: Option<String>,
    route_reason: Option<String>,
    quality_score: f64,
    value_score: f64,
    confidence: f64,
    retry_count: u32,
}

#[derive(Debug, Clone)]
struct NotionTreeConfig {
    route_min_confidence: f64,
    unknown_category: String,
    default_categories: Vec<String>,
    timezone: String,
    category_growth_limit: u32,
}

#[derive(Debug, Clone)]
struct TreeItemNode {
    id: String,
    page_id: String,
    parent_page_id: String,
    category_name: Option<String>,
    category_history_json: String,
    meta_block_id: Option<String>,
    summary_block_id: Option<String>,
    key_points_block_id: Option<String>,
    source_link_block_id: Option<String>,
    tags_block_id: Option<String>,
    images_block_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ParsedClassificationPayload {
    labels: Vec<String>,
    tags: Vec<String>,
    key_points: Vec<String>,
    final_category: Option<String>,
    route_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct StructuredContent {
    summary: String,
    key_points: Vec<String>,
    tags: Vec<String>,
    quality_score: f64,
    quality_state: String,
    degraded_fields: Vec<String>,
    content_source: String,
    usage: Option<ModelUsageRecord>,
}

#[derive(Debug, Clone)]
struct StructuredContentConfig {
    enabled: bool,
    api_key: Option<String>,
    model: String,
    max_calls_per_run: usize,
    reduced_max_calls_per_run: usize,
    budget_limit_cny: f64,
    budget_degrade_ratio: f64,
    budget_fuse_ratio: f64,
    deep_fetch_enabled: bool,
    fetch_timeout_ms: u64,
}

#[derive(Debug, Clone)]
struct StructuredContentState {
    config: StructuredContentConfig,
    calls_used: usize,
    budget_usage_ratio: f64,
    model_call_cap: usize,
    fuse_active: bool,
    reduce_active: bool,
}

#[derive(Debug, Clone)]
struct ModelUsageRecord {
    provider: String,
    model: String,
    purpose: String,
    tokens_in: u64,
    tokens_out: u64,
    cost_cny: f64,
}

#[derive(Debug, Clone)]
struct BudgetGuardPolicy {
    usage_ratio: f64,
    model_call_cap: usize,
    fuse_active: bool,
    reduce_active: bool,
}

pub async fn sync_pending_with_conn<C: NotionClient>(
    conn: &Connection,
    client: &C,
    limit: usize,
) -> Result<SyncRunSummary> {
    sync_pending_with_conn_with_options(conn, client, limit, 400).await
}

pub async fn sync_pending_with_conn_with_options<C: NotionClient>(
    conn: &Connection,
    client: &C,
    limit: usize,
    sleep_ms: u64,
) -> Result<SyncRunSummary> {
    let rows = load_pending_rows(conn, limit)?;
    let mut summary = SyncRunSummary {
        scanned: rows.len() as u32,
        attempted: 0,
        succeeded: 0,
        failed: 0,
        requeued: 0,
        dead_lettered: 0,
    };
    let retry_policy = RetryPolicy::default();

    for row in rows {
        summary.attempted += 1;
        let record = NotionRecord {
            normalized_item_id: row.normalized_item_id.clone(),
            title: row.title.clone(),
            summary: row.summary.clone(),
            labels: row.labels.clone(),
            source: row.source.clone(),
            source_url: row.source_url.clone(),
            published_at: row.published_at.clone(),
            confidence: row.confidence,
            quality_score: row.quality_score,
            value_score: row.value_score,
        };

        let upsert_result = client.upsert_one(&record).await;
        match upsert_result {
            Ok(()) => {
                summary.succeeded += 1;
                state_persistence::mark_sync_success(conn, &row.sync_record_id, None)?;
            }
            Err(e) => {
                summary.failed += 1;
                let sync_error_code = state_persistence::classify_sync_error_code(&e);
                let next_retry_count = row.retry_count + 1;
                if next_retry_count < retry_policy.max_attempts {
                    summary.requeued += 1;
                    let backoff_ms = retry_policy.backoff_ms(row.retry_count);
                    state_persistence::mark_sync_retry(
                        conn,
                        &row.sync_record_id,
                        next_retry_count,
                        backoff_ms,
                        &e,
                        sync_error_code,
                    )?;
                } else {
                    summary.dead_lettered += 1;
                    state_persistence::mark_sync_failed(
                        conn,
                        &row.sync_record_id,
                        next_retry_count,
                        &e,
                        sync_error_code,
                    )?;
                    state_persistence::insert_dead_letter(
                        conn,
                        &row.sync_record_id,
                        "notion_sync",
                        sync_error_code,
                        &format!("notion sync failed: {}", e),
                    )?;
                }
            }
        }

        if sleep_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
    }

    Ok(summary)
}

pub async fn sync_pending_page_tree_with_conn<C: NotionTreeClient>(
    conn: &Connection,
    client: &C,
    root_page_id: &str,
    limit: usize,
) -> Result<SyncRunSummary> {
    sync_pending_page_tree_with_conn_with_options(conn, client, root_page_id, limit, 400).await
}

pub async fn sync_pending_page_tree_with_conn_with_options<C: NotionTreeClient>(
    conn: &Connection,
    client: &C,
    root_page_id: &str,
    limit: usize,
    sleep_ms: u64,
) -> Result<SyncRunSummary> {
    if root_page_id.trim().is_empty() {
        return Err(BackendError::Validation(
            "NOTION_ROOT_PAGE_ID is required".to_string(),
        ));
    }
    let config = load_tree_config(conn)?;
    let content_config = ai_content::load_structured_content_config();
    let budget_policy = ai_content::resolve_budget_guard_policy(conn, &content_config)?;
    let mut content_state = StructuredContentState {
        config: content_config,
        calls_used: 0,
        budget_usage_ratio: budget_policy.usage_ratio,
        model_call_cap: budget_policy.model_call_cap,
        fuse_active: budget_policy.fuse_active,
        reduce_active: budget_policy.reduce_active,
    };
    let mut known_custom_categories =
        tree_builder::load_known_custom_categories(conn, client, root_page_id, &config).await?;
    let rows = load_pending_rows(conn, limit)?;
    let mut summary = SyncRunSummary {
        scanned: rows.len() as u32,
        attempted: 0,
        succeeded: 0,
        failed: 0,
        requeued: 0,
        dead_lettered: 0,
    };
    let retry_policy = RetryPolicy::default();

    for row in rows {
        summary.attempted += 1;
        let upsert_result = tree_builder::upsert_tree_item(
            conn,
            client,
            root_page_id,
            &config,
            &row,
            &mut known_custom_categories,
            &mut content_state,
        )
        .await;
        match upsert_result {
            Ok(page_id) => {
                summary.succeeded += 1;
                state_persistence::mark_sync_success(conn, &row.sync_record_id, Some(&page_id))?;
            }
            Err(e) => {
                summary.failed += 1;
                let sync_error_code = state_persistence::classify_sync_error_code(&e);
                let next_retry_count = row.retry_count + 1;
                if state_persistence::is_quality_gate_error(&e) {
                    summary.dead_lettered += 1;
                    state_persistence::mark_sync_failed(
                        conn,
                        &row.sync_record_id,
                        next_retry_count,
                        &e,
                        "MOD-3101",
                    )?;
                    state_persistence::insert_dead_letter(
                        conn,
                        &row.sync_record_id,
                        "notion_quality_gate",
                        "MOD-3101",
                        &format!("quality gate failed: {}", e),
                    )?;
                } else if next_retry_count < retry_policy.max_attempts {
                    summary.requeued += 1;
                    let backoff_ms = retry_policy.backoff_ms(row.retry_count);
                    state_persistence::mark_sync_retry(
                        conn,
                        &row.sync_record_id,
                        next_retry_count,
                        backoff_ms,
                        &e,
                        sync_error_code,
                    )?;
                } else {
                    summary.dead_lettered += 1;
                    state_persistence::mark_sync_failed(
                        conn,
                        &row.sync_record_id,
                        next_retry_count,
                        &e,
                        sync_error_code,
                    )?;
                    state_persistence::insert_dead_letter(
                        conn,
                        &row.sync_record_id,
                        "notion_sync_page_tree",
                        sync_error_code,
                        &format!("notion page_tree sync failed: {}", e),
                    )?;
                }
            }
        }
        if sleep_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
        }
    }

    Ok(summary)
}

fn load_pending_rows(conn: &Connection, limit: usize) -> Result<Vec<PendingSyncRow>> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    seed_publish_tasks_if_missing_for_sync(conn, &now)?;
    let mut stmt = conn.prepare(
        "SELECT
            s.id,
            n.id AS normalized_item_id,
            IFNULL(src.source, 'wechat'),
            src.source_url,
            src.published_at,
            src.cover_url,
            IFNULL(src.image_urls_json, '[]'),
            IFNULL(src.title, 'Untitled'),
            IFNULL(src.content_raw, ''),
            IFNULL(a.summary_text, n.text_clean),
            IFNULL(a.classification_json, '[]'),
            IFNULL(a.quality_score, n.quality_score),
            IFNULL(a.value_score, n.value_score),
            IFNULL(a.c_score, 0.0),
            p.attempt_count
         FROM publish_tasks p
         JOIN normalized_items n ON n.source_item_id = p.item_id
         JOIN sync_records s ON s.normalized_item_id = n.id AND s.target = 'notion'
         LEFT JOIN source_items src ON src.id = n.source_item_id
         LEFT JOIN analysis_results a ON a.normalized_item_id = n.id
         LEFT JOIN source_state ss ON ss.item_id = n.source_item_id
         WHERE p.state IN ('pending','failed','processing')
           AND (p.next_retry_at IS NULL OR p.next_retry_at <= ?1)
           AND COALESCE(ss.active_state, 'active') = 'active'
         ORDER BY p.updated_at ASC
         LIMIT ?2",
    )?;

    let mapped = stmt.query_map(params![now, limit as i64], |row| {
        let classification_json: String = row.get(10)?;
        let parsed = parse_classification_payload(&classification_json);
        let image_urls_json: String = row.get(6)?;
        let image_urls = serde_json::from_str::<Vec<String>>(&image_urls_json).unwrap_or_default();
        let summary: String = row.get(9)?;
        let summary_text = if summary.trim().is_empty() {
            "-".to_string()
        } else {
            summary
        };
        let content_raw = row.get::<_, String>(8)?;
        Ok(PendingSyncRow {
            sync_record_id: row.get(0)?,
            normalized_item_id: row.get(1)?,
            source: row.get(2)?,
            source_url: row.get(3)?,
            content_raw,
            published_at: row.get(4)?,
            cover_url: row.get(5)?,
            image_urls,
            title: row.get(7)?,
            summary: summary_text,
            labels: parsed.labels,
            tags: parsed.tags,
            key_points: parsed.key_points,
            final_category: parsed.final_category,
            route_reason: parsed.route_reason,
            quality_score: row.get(11)?,
            value_score: row.get(12)?,
            confidence: row.get(13)?,
            retry_count: row.get::<_, i64>(14)? as u32,
        })
    })?;

    let mut out = Vec::new();
    for item in mapped {
        out.push(item?);
    }
    Ok(out)
}

fn seed_publish_tasks_if_missing_for_sync(conn: &Connection, now: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO publish_tasks(
            id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
         )
         SELECT
            ('pt_' || src.id) AS id,
            src.id AS item_id,
            CASE
              WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'ignored'
              WHEN q.state = 'rejected' THEN 'ignored'
              WHEN src.source = 'xhs' AND q.state IN ('pending', 'processing') THEN 'ignored'
              WHEN COALESCE(s.sync_state, 'pending') = 'success' THEN 'published'
              WHEN COALESCE(s.sync_state, 'pending') IN ('failed', 'retry') THEN 'failed'
              ELSE 'pending'
            END AS state,
            COALESCE(s.retry_count, 0) AS attempt_count,
            s.next_retry_at,
            CASE
              WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source_inactive'
              WHEN q.state = 'rejected' THEN 'review_rejected'
              WHEN src.source = 'xhs' AND q.state IN ('pending', 'processing') THEN 'await_review'
              ELSE s.last_error_code
            END AS error_code,
            CASE
              WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source marked inactive'
              WHEN q.state = 'rejected' THEN 'manual review rejected'
              WHEN src.source = 'xhs' AND q.state IN ('pending', 'processing') THEN 'await manual review'
              ELSE s.last_error_message
            END AS error_message,
            COALESCE(s.created_at, ?1),
            ?1
         FROM sync_records s
         JOIN normalized_items n ON n.id = s.normalized_item_id
         JOIN source_items src ON src.id = n.source_item_id
         LEFT JOIN review_queue q ON q.normalized_item_id = n.id
         LEFT JOIN source_state ss ON ss.item_id = src.id
         WHERE s.target = 'notion'
         ON CONFLICT(id) DO NOTHING",
        params![now],
    )?;
    conn.execute(
        "UPDATE publish_tasks
         SET state='failed',
             attempt_count=attempt_count + 1,
             next_retry_at=NULL,
             error_code=CASE
               WHEN COALESCE(error_code, '') = '' THEN 'publish_stuck'
               ELSE error_code
             END,
             error_message=CASE
               WHEN COALESCE(error_message, '') = '' THEN 'processing timeout'
               ELSE error_message
             END,
             updated_at=?1
         WHERE state='processing'
           AND datetime(updated_at) <= datetime(?1, '-15 minutes')",
        params![now],
    )?;
    conn.execute(
        "UPDATE publish_tasks
         SET state='pending',
             attempt_count=COALESCE(src.retry_count, 0),
             next_retry_at=src.next_retry_at,
             error_code=src.last_error_code,
             error_message=src.last_error_message,
             updated_at=?1
         FROM (
            SELECT
              s.id AS sync_record_id,
              ('pt_' || origin.id) AS publish_task_id,
              s.retry_count,
              s.next_retry_at,
              s.last_error_code,
              s.last_error_message,
              origin.source AS source,
              q.state AS review_state,
              COALESCE(ss.active_state, 'active') AS active_state
            FROM sync_records s
            JOIN normalized_items n ON n.id = s.normalized_item_id
            JOIN source_items origin ON origin.id = n.source_item_id
            LEFT JOIN review_queue q ON q.normalized_item_id = n.id
            LEFT JOIN source_state ss ON ss.item_id = origin.id
            WHERE s.target='notion'
              AND s.sync_state IN ('pending','retry')
         ) AS src
         WHERE publish_tasks.id = src.publish_task_id
           AND (
             publish_tasks.state = 'published'
             OR (
               publish_tasks.state = 'ignored'
               AND COALESCE(publish_tasks.error_code, '') = ''
             )
           )
           AND src.active_state = 'active'
           AND COALESCE(src.review_state, '') != 'rejected'
           AND NOT (src.source = 'xhs' AND COALESCE(src.review_state, '') IN ('pending','processing'))",
        params![now],
    )?;
    Ok(())
}

fn load_tree_config(conn: &Connection) -> Result<NotionTreeConfig> {
    let route_min_confidence = config::env_or_db_f64(
        conn,
        "NOTION_TREE_ROUTE_MIN_CONFIDENCE",
        "notion.tree.route_min_confidence",
    )?
    .unwrap_or(0.72)
    .clamp(0.0, 1.0);
    let unknown_category = config::env_or_db_string(
        conn,
        "NOTION_TREE_UNKNOWN_CATEGORY",
        "notion.tree.unknown_category",
    )?
    .unwrap_or_else(|| "Inbox".to_string());
    let default_categories = config::env_or_db_string(
        conn,
        "NOTION_TREE_DEFAULT_CATEGORIES",
        "notion.tree.default_categories",
    )?
    .unwrap_or_else(|| "学习|工作|生活|健康|财务|灵感|Inbox".to_string())
    .split('|')
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
    .collect::<Vec<_>>();
    let timezone = config::env_or_db_string(conn, "NOTION_TREE_TIMEZONE", "notion.tree.timezone")?
        .unwrap_or_else(|| "Asia/Shanghai".to_string());
    let fixed_first_level_categories = config::env_bool("NOTION_TREE_FIXED_FIRST_LEVEL", true);
    // Fail fast on invalid timezone instead of silently falling back.
    week_key_and_title(Utc::now(), &timezone).map_err(BackendError::Validation)?;
    let mut category_growth_limit = config::env_or_db_i64(
        conn,
        "NOTION_TREE_CATEGORY_GROWTH_LIMIT",
        "notion.tree.category_growth_limit",
    )?
    .unwrap_or(32);
    if fixed_first_level_categories {
        category_growth_limit = 0;
    }
    if category_growth_limit < 0 {
        return Err(BackendError::Validation(
            "NOTION_TREE_CATEGORY_GROWTH_LIMIT must be >= 0".to_string(),
        ));
    }

    Ok(NotionTreeConfig {
        route_min_confidence,
        unknown_category,
        default_categories,
        timezone,
        category_growth_limit: category_growth_limit as u32,
    })
}

fn parse_published_at(raw: &Option<String>) -> Option<DateTime<Utc>> {
    let value = raw.as_ref()?.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = ChronoNaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S") {
        // published_at without timezone is treated as Asia/Shanghai local time.
        let ts = naive.and_utc() - Duration::hours(8);
        return Some(ts);
    }
    None
}

fn parse_classification_payload(raw: &str) -> ParsedClassificationPayload {
    let mut out = ParsedClassificationPayload::default();
    let parsed = serde_json::from_str::<Value>(raw).ok();
    match parsed {
        Some(Value::Array(arr)) => {
            out.labels = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect();
        }
        Some(Value::Object(map)) => {
            out.labels = extract_string_list(map.get("labels"));
            out.tags = extract_string_list(map.get("tags"));
            out.key_points = extract_string_list(map.get("key_points"));
            out.final_category = map
                .get("final_category")
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            out.route_reason = map
                .get("route_reason")
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            if out.labels.is_empty() {
                out.labels = extract_string_list(map.get("classification"));
            }
        }
        _ => {}
    }
    if out.labels.is_empty() {
        out.labels.push("未分类".to_string());
    }
    if out.tags.is_empty() {
        out.tags = out.labels.clone();
    }
    if out.key_points.len() > 5 {
        out.key_points.truncate(5);
    }
    out
}

pub(super) fn extract_string_list(value: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(Value::Array(arr)) = value {
        for item in arr {
            if let Some(text) = item.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    out
}

pub(super) fn normalize_text_output(raw: &str, max_chars: usize) -> String {
    let collapsed = raw
        .replace('\r', "\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = collapsed.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed.chars().take(max_chars).collect::<String>()
}

pub(super) fn normalize_string_list(
    raw: &[String],
    max_items: usize,
    max_chars_each: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    for item in raw.iter().filter(|v| !v.trim().is_empty()) {
        out.push(item.trim().chars().take(max_chars_each).collect::<String>());
        if out.len() >= max_items {
            break;
        }
    }
    dedupe_string_vec(&mut out);
    out
}

pub(super) fn dedupe_string_vec(values: &mut Vec<String>) {
    let mut seen = HashSet::<String>::new();
    values.retain(|item| {
        let key = item.trim().to_ascii_lowercase();
        !key.is_empty() && seen.insert(key)
    });
}

pub(super) fn stable_hash(input: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in input.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

#[allow(private_interfaces)]
pub(super) fn render_status_metadata(
    row: &PendingSyncRow,
    category: &str,
    route_reason: Option<&str>,
    content: &StructuredContent,
) -> String {
    let _ = (row, category, route_reason, content);
    String::new()
}

#[allow(dead_code)]
fn parse_dt(v: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S").ok()
}

#[cfg(test)]
mod tests {
    use super::ai_content::model_skip_reason;
    use super::*;

    fn test_content_state() -> StructuredContentState {
        StructuredContentState {
            config: StructuredContentConfig {
                enabled: true,
                api_key: Some("test_key".to_string()),
                model: "qwen-flash".to_string(),
                max_calls_per_run: 20,
                reduced_max_calls_per_run: 10,
                budget_limit_cny: 100.0,
                budget_degrade_ratio: 0.8,
                budget_fuse_ratio: 1.0,
                deep_fetch_enabled: false,
                fetch_timeout_ms: 1_000,
            },
            calls_used: 0,
            budget_usage_ratio: 0.0,
            model_call_cap: 20,
            fuse_active: false,
            reduce_active: false,
        }
    }

    #[test]
    fn model_skip_reason_returns_budget_fused_at_full_budget() {
        let mut state = test_content_state();
        state.fuse_active = true;

        assert_eq!(model_skip_reason(&state), Some("budget_fused"));
    }

    #[test]
    fn model_skip_reason_returns_reduced_cap_when_reduced_mode_hits_cap() {
        let mut state = test_content_state();
        state.reduce_active = true;
        state.model_call_cap = 4;
        state.calls_used = 4;

        assert_eq!(model_skip_reason(&state), Some("budget_reduced_cap"));
    }

    #[test]
    fn model_skip_reason_allows_model_when_within_cap() {
        let state = test_content_state();
        assert_eq!(model_skip_reason(&state), None);
    }
}
