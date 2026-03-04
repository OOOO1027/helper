use chrono::{Local, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::process::Command;
use tracing::warn;
use uuid::Uuid;

use crate::app_core::{
    AppCore, CollectResult, CollectionInsight, DashboardMetrics, DateRange, ImportPersistSummary,
    ImportSummary, JobRunResult, PageReq, Paged, PipelineExecuteResult, PipelinePreviewReq,
    PipelinePreviewResult, RetryResult, ReviewFilter, ReviewItem, ReviewPatch, SyncDailyStat,
    SyncLog,
};
use crate::budget_guard::BudgetStatus;
use crate::config;
use crate::env_runtime::env_with_shell_fallback;
use crate::storage::db::{open_sqlcipher, run_migrations, DbConfig};
use crate::sync_notion::notion_api::NotionHttpClient;
use crate::sync_notion::service::{
    sync_pending_page_tree_with_conn_with_options, sync_pending_with_conn_with_options,
    SyncRunSummary,
};
use crate::sync_notion::tree::week_key_and_title;
use crate::{BackendError, Result};

pub mod publish;
pub use publish::{
    get_publish_history, get_publish_history_with_filters, get_publish_queue,
    publish_approved_to_notion,
};

#[cfg(feature = "tauri-integration")]
pub mod tauri_api;

// ── IPC request / response types ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportWechatFilesReq {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectReq {
    pub source: String,
    pub since: Option<String>,
    pub until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReviewItemsReq {
    pub filter: ReviewFilter,
    pub page: PageReq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReviewItemReq {
    pub id: String,
    pub patch: ReviewPatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryFailedItemsReq {
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSyncLogsReq {
    pub range: DateRange,
    pub page: PageReq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSyncDailyStatsReq {
    pub range: DateRange,
    pub page: PageReq,
    pub job_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAiUsageMonthlyReq {
    pub month: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunNotionSyncReq {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetIngestQueueReq {
    pub page: PageReq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReviewQueueReq {
    pub filter: ReviewFilter,
    pub page: PageReq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishApprovedReq {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPublishQueueReq {
    pub page: PageReq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPublishHistoryReq {
    pub range: DateRange,
    pub page: PageReq,
    pub state: Option<String>,
    pub sync_mode: Option<String>,
    pub retryable: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct PublishHistoryFilters {
    pub state: Option<String>,
    pub sync_mode: Option<String>,
    pub retryable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkSourceInactiveReq {
    pub item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalToolStatus {
    pub textutil: bool,
    pub pdftotext: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestQueueItem {
    pub source_item_id: String,
    pub source: String,
    pub title: String,
    pub collected_at: String,
    pub review_state: String,
    pub publish_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishTaskItem {
    pub id: String,
    pub item_id: String,
    pub title: String,
    pub state: String,
    pub attempt_count: u32,
    pub next_retry_at: Option<String>,
    pub error_code: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishHistoryItem {
    pub id: String,
    pub state: String,
    pub error_code: Option<String>,
    pub created_at: String,
    pub success_count: u32,
    pub fail_count: u32,
    pub pipeline_stage: Option<String>,
    pub sync_mode: Option<String>,
    pub retryable: Option<bool>,
    pub category: Option<String>,
    pub week_key: Option<String>,
    pub route_reason: Option<String>,
    pub conclusion_summary: Option<String>,
    pub key_points: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub cover_url: Option<String>,
    pub image_urls: Option<Vec<String>>,
    pub quality_state: Option<String>,
    pub quality_score: Option<f64>,
    pub degraded_fields: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    pub request_id: String,
    pub attempted: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub requeued: u32,
    pub dead_lettered: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceStateMarkResult {
    pub item_id: String,
    pub active_state: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionSyncConfigSnapshot {
    pub sync_mode: String,
    pub notion_token_configured: bool,
    pub notion_database_id_configured: bool,
    pub notion_root_page_id_configured: bool,
    pub timezone: String,
    pub category_growth_limit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageModelItem {
    pub model: String,
    pub calls: u32,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_cny: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageDailyItem {
    pub day: String,
    pub calls: u32,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_cny: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageMonthlySummary {
    pub month: String,
    pub budget_limit_cny: f64,
    pub used_cny: f64,
    pub usage_ratio: f64,
    pub remaining_cny: f64,
    pub calls: u32,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub avg_cost_per_call_cny: f64,
    pub models: Vec<AiUsageModelItem>,
    pub daily: Vec<AiUsageDailyItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub ok: bool,
    pub request_id: String,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

// ── Shared low-level helpers (pub(super) so submodules can use them) ──────────

pub(super) fn open_conn_from_env() -> Result<rusqlite::Connection> {
    let db_path = required_env("HELPER_DB_PATH")?;
    let db_key = required_env("HELPER_DB_KEY")?;
    let conn = open_sqlcipher(&DbConfig {
        path: db_path,
        key: db_key,
    })?;
    run_migrations(&conn)?;
    Ok(conn)
}

pub(super) fn page_bounds(page: &PageReq) -> Result<(i64, i64)> {
    if page.page == 0 || page.page_size == 0 {
        return Err(BackendError::Validation(
            "page and page_size must be positive".to_string(),
        ));
    }
    let page_size = page.page_size as i64;
    let offset = ((page.page - 1) * page.page_size) as i64;
    Ok((page_size, offset))
}

pub(super) fn now_ts() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ── Business functions ────────────────────────────────────────────────────────

pub fn run_daily_job(app: &AppCore) -> Result<JobRunResult> {
    app.run_daily_job()
}

pub fn import_wechat_files(app: &AppCore, paths: Vec<String>) -> Result<ImportSummary> {
    app.import_wechat_files(&paths)
}

pub fn collect(
    app: &AppCore,
    source: String,
    since: Option<String>,
    until: Option<String>,
) -> Result<CollectResult> {
    app.collect(&source, since.as_deref(), until.as_deref())
}

pub fn get_dashboard_metrics(app: &AppCore) -> Result<DashboardMetrics> {
    app.get_dashboard_metrics()
}

pub fn get_collection_insight(app: &AppCore) -> Result<CollectionInsight> {
    app.get_collection_insight()
}

pub fn get_review_items(
    app: &AppCore,
    filter: ReviewFilter,
    page: PageReq,
) -> Result<Paged<ReviewItem>> {
    app.get_review_items(filter, page)
}

pub fn update_review_item(app: &AppCore, id: String, patch: ReviewPatch) -> Result<ReviewItem> {
    app.update_review_item(&id, patch)
}

pub fn retry_failed_items(app: &AppCore, ids: Vec<String>) -> Result<RetryResult> {
    app.retry_failed_items(&ids)
}

pub fn retry_dead_letters(app: &AppCore, ids: Vec<String>) -> Result<RetryResult> {
    let mut conn = open_conn_from_env()?;
    app.retry_dead_letters_with_conn(&mut conn, &ids)
}

pub fn get_budget_status(app: &AppCore) -> Result<BudgetStatus> {
    app.get_budget_status()
}

pub fn get_ai_usage_monthly_summary(
    _app: &AppCore,
    month: Option<String>,
) -> Result<AiUsageMonthlySummary> {
    let conn = open_conn_from_env()?;
    let month_key = normalize_month_key(month)?;
    let budget_limit_cny = read_monthly_budget_limit_cny();

    let (used_cny, calls, tokens_in, tokens_out): (f64, i64, i64, i64) = conn.query_row(
        "SELECT
            COALESCE(SUM(cost_cny), 0.0),
            COUNT(1),
            COALESCE(SUM(tokens_in), 0),
            COALESCE(SUM(tokens_out), 0)
         FROM budget_ledger
         WHERE day LIKE (?1 || '%')",
        params![&month_key],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;

    let mut model_stmt = conn.prepare(
        "SELECT
            COALESCE(model, 'unknown') AS model,
            COUNT(1) AS calls,
            COALESCE(SUM(tokens_in), 0) AS tokens_in,
            COALESCE(SUM(tokens_out), 0) AS tokens_out,
            COALESCE(SUM(cost_cny), 0.0) AS cost_cny
         FROM budget_ledger
         WHERE day LIKE (?1 || '%')
         GROUP BY COALESCE(model, 'unknown')
         ORDER BY cost_cny DESC, calls DESC",
    )?;
    let mut models = Vec::new();
    let mut model_rows = model_stmt.query(params![&month_key])?;
    while let Some(row) = model_rows.next()? {
        models.push(AiUsageModelItem {
            model: row.get(0)?,
            calls: row.get::<_, i64>(1)?.max(0) as u32,
            tokens_in: row.get::<_, i64>(2)?.max(0) as u64,
            tokens_out: row.get::<_, i64>(3)?.max(0) as u64,
            cost_cny: row.get::<_, f64>(4)?.max(0.0),
        });
    }

    let mut daily_stmt = conn.prepare(
        "SELECT
            day,
            COUNT(1) AS calls,
            COALESCE(SUM(tokens_in), 0) AS tokens_in,
            COALESCE(SUM(tokens_out), 0) AS tokens_out,
            COALESCE(SUM(cost_cny), 0.0) AS cost_cny
         FROM budget_ledger
         WHERE day LIKE (?1 || '%')
         GROUP BY day
         ORDER BY day DESC",
    )?;
    let mut daily = Vec::new();
    let mut daily_rows = daily_stmt.query(params![&month_key])?;
    while let Some(row) = daily_rows.next()? {
        daily.push(AiUsageDailyItem {
            day: row.get(0)?,
            calls: row.get::<_, i64>(1)?.max(0) as u32,
            tokens_in: row.get::<_, i64>(2)?.max(0) as u64,
            tokens_out: row.get::<_, i64>(3)?.max(0) as u64,
            cost_cny: row.get::<_, f64>(4)?.max(0.0),
        });
    }

    let safe_calls = calls.max(0) as u32;
    let safe_used = used_cny.max(0.0);
    let usage_ratio = if budget_limit_cny <= 0.0 {
        0.0
    } else {
        (safe_used / budget_limit_cny).clamp(0.0, 10.0)
    };
    let remaining_cny = (budget_limit_cny - safe_used).max(0.0);
    let avg_cost_per_call_cny = if safe_calls == 0 {
        0.0
    } else {
        safe_used / safe_calls as f64
    };

    Ok(AiUsageMonthlySummary {
        month: month_key,
        budget_limit_cny,
        used_cny: safe_used,
        usage_ratio,
        remaining_cny,
        calls: safe_calls,
        tokens_in: tokens_in.max(0) as u64,
        tokens_out: tokens_out.max(0) as u64,
        avg_cost_per_call_cny,
        models,
        daily,
    })
}

pub fn get_sync_logs(app: &AppCore, range: DateRange, page: PageReq) -> Result<Paged<SyncLog>> {
    let conn = open_conn_from_env()?;
    app.get_sync_logs_with_conn(&conn, range, page)
}

pub fn get_sync_daily_stats(
    app: &AppCore,
    range: DateRange,
    page: PageReq,
    job_type: Option<String>,
    status: Option<String>,
) -> Result<Paged<SyncDailyStat>> {
    let conn = open_conn_from_env()?;
    app.get_sync_daily_stats_with_conn(&conn, range, page, job_type, status)
}

pub fn ingest_xhs_incremental(app: &AppCore) -> Result<CollectResult> {
    collect(app, "xhs".to_string(), None, None)
}

pub fn get_ingest_queue(_app: &AppCore, page: PageReq) -> Result<Paged<IngestQueueItem>> {
    let conn = open_conn_from_env()?;
    publish::seed_publish_tasks_if_missing(&conn)?;
    let (page_size, offset) = page_bounds(&page)?;

    let total: i64 = conn.query_row(
        "SELECT COUNT(1) FROM source_items WHERE source = 'xhs'",
        [],
        |r| r.get(0),
    )?;

    let mut stmt = conn.prepare(
        "SELECT
            s.id AS source_item_id,
            s.source,
            COALESCE(NULLIF(TRIM(s.title), ''), NULLIF(TRIM(substr(s.content_raw, 1, 120)), '')) AS title,
            s.collected_at,
            CASE
              WHEN q.state IS NOT NULL THEN q.state
              WHEN n.id IS NULL THEN 'pending'
              ELSE 'done'
            END AS review_state,
            CASE
              WHEN pt.state IS NOT NULL THEN pt.state
              WHEN COALESCE(sr.sync_state, 'pending') = 'success' THEN 'published'
              WHEN COALESCE(sr.sync_state, 'pending') = 'failed' THEN 'failed'
              WHEN COALESCE(sr.sync_state, 'pending') = 'retry' THEN 'retry'
              ELSE 'pending'
            END AS publish_state
         FROM source_items s
         LEFT JOIN normalized_items n ON n.source_item_id = s.id
         LEFT JOIN review_queue q ON q.normalized_item_id = n.id
         LEFT JOIN sync_records sr ON sr.normalized_item_id = n.id AND sr.target = 'notion'
         LEFT JOIN publish_tasks pt ON pt.item_id = s.id
         WHERE s.source = 'xhs'
         ORDER BY s.collected_at DESC
         LIMIT ?1 OFFSET ?2",
    )?;

    let mut rows = stmt.query(params![page_size, offset])?;
    let mut items = Vec::new();
    while let Some(row) = rows.next()? {
        items.push(IngestQueueItem {
            source_item_id: row.get(0)?,
            source: row.get(1)?,
            title: row
                .get::<_, Option<String>>(2)?
                .unwrap_or_else(|| "未命名内容".to_string()),
            collected_at: row.get(3)?,
            review_state: row.get(4)?,
            publish_state: row.get(5)?,
        });
    }

    Ok(Paged {
        items,
        page: page.page,
        page_size: page.page_size,
        total: total.max(0) as u64,
    })
}

pub fn get_review_queue(
    app: &AppCore,
    filter: ReviewFilter,
    page: PageReq,
) -> Result<Paged<ReviewItem>> {
    let conn = open_conn_from_env()?;
    app.get_review_items_with_conn(&conn, filter, page)
}

pub fn mark_source_inactive(_app: &AppCore, item_id: String) -> Result<SourceStateMarkResult> {
    if item_id.trim().is_empty() {
        return Err(BackendError::Validation(
            "item_id must not be empty".to_string(),
        ));
    }
    let conn = open_conn_from_env()?;
    let exists: Option<String> = conn
        .query_row(
            "SELECT id FROM source_items WHERE id = ?1",
            params![&item_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(BackendError::Validation(format!(
            "source item not found: {item_id}"
        )));
    }
    let now = now_ts();
    conn.execute(
        "INSERT INTO source_state(item_id, active_state, last_seen_at, updated_at)
         VALUES (?1, 'inactive', ?2, ?2)
         ON CONFLICT(item_id) DO UPDATE SET
            active_state='inactive',
            last_seen_at=excluded.last_seen_at,
            updated_at=excluded.updated_at",
        params![&item_id, &now],
    )?;
    conn.execute(
        "UPDATE publish_tasks
         SET state='ignored',
             error_code='source_inactive',
             error_message='source marked inactive',
             updated_at=?2
         WHERE item_id = ?1",
        params![&item_id, &now],
    )?;
    conn.execute(
        "UPDATE sync_records
         SET sync_state='failed',
             retry_count=retry_count + 1,
             next_retry_at=NULL,
             last_error_code='source_inactive',
             last_error_message='source marked inactive',
             updated_at=?2
         WHERE normalized_item_id IN (
            SELECT id FROM normalized_items WHERE source_item_id = ?1
         )
           AND target='notion'
           AND sync_state IN ('pending', 'retry')",
        params![&item_id, &now],
    )?;

    Ok(SourceStateMarkResult {
        item_id,
        active_state: "inactive".to_string(),
        updated_at: now,
    })
}

pub fn run_pipeline_preview(
    app: &AppCore,
    req: PipelinePreviewReq,
) -> Result<PipelinePreviewResult> {
    app.preview_pipeline(req)
}

pub fn run_pipeline_persist(
    app: &AppCore,
    req: PipelinePreviewReq,
) -> Result<PipelineExecuteResult> {
    let mut conn = open_conn_from_env()?;
    app.execute_pipeline_with_conn(&mut conn, req)
}

pub fn import_wechat_and_persist(
    app: &AppCore,
    paths: Vec<String>,
) -> Result<ImportPersistSummary> {
    let mut conn = open_conn_from_env()?;
    app.import_wechat_and_persist_with_conn(&mut conn, &paths)
}

pub async fn run_notion_sync_once(_app: &AppCore, limit: Option<usize>) -> Result<SyncRunSummary> {
    let conn = open_conn_from_env()?;
    let notion_token = required_env("NOTION_TOKEN")?;

    let mode = read_notion_sync_mode(Some(&conn))?;
    let cfg_limit = read_app_config_i64(&conn, "notion.sync.default_limit")?.unwrap_or(50);
    let cfg_sleep_ms = read_app_config_i64(&conn, "notion.sync.sleep_ms")?.unwrap_or(400);
    let raw_limit = limit.unwrap_or_else(|| {
        if cfg_limit <= 0 {
            0
        } else {
            cfg_limit as usize
        }
    });
    let effective_limit = sanitize_limit(raw_limit);
    let effective_sleep_ms = sanitize_sleep_ms(cfg_sleep_ms);
    let started_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sync_result = match mode.as_str() {
        "database" => {
            let notion_database_id = required_env("NOTION_DATABASE_ID")?;
            let client = NotionHttpClient::new(notion_token.clone(), notion_database_id)?;
            sync_pending_with_conn_with_options(&conn, &client, effective_limit, effective_sleep_ms)
                .await
        }
        "page_tree" => {
            let root_page_id = required_env("NOTION_ROOT_PAGE_ID")?;
            let client = NotionHttpClient::new_page_tree(notion_token.clone())?;
            sync_pending_page_tree_with_conn_with_options(
                &conn,
                &client,
                &root_page_id,
                effective_limit,
                effective_sleep_ms,
            )
            .await
        }
        _ => Err(BackendError::Validation(format!(
            "NOTION_SYNC_MODE must be 'page_tree' or 'database', got '{}'",
            mode
        ))),
    };
    match &sync_result {
        Ok(summary) => {
            if let Err(e) = record_notion_sync_job_run(
                &conn,
                &started_at,
                effective_limit,
                effective_sleep_ms,
                Some(summary),
                None,
            ) {
                warn!(error = %e, "persist notion_sync_once job run failed");
            }
        }
        Err(err) => {
            if let Err(e) = record_notion_sync_job_run(
                &conn,
                &started_at,
                effective_limit,
                effective_sleep_ms,
                None,
                Some(err),
            ) {
                warn!(error = %e, "persist failed notion_sync_once job run failed");
            }
        }
    }
    sync_result
}

pub fn get_local_tool_status() -> Result<LocalToolStatus> {
    Ok(LocalToolStatus {
        textutil: command_available("textutil", &["-help"])?,
        pdftotext: command_available("pdftotext", &["-v"])?,
    })
}

pub fn get_notion_sync_config_snapshot() -> Result<NotionSyncConfigSnapshot> {
    let conn = try_open_app_config_conn();
    let sync_mode = read_notion_sync_mode(conn.as_ref())?;
    let timezone = resolve_string_config(
        conn.as_ref(),
        "NOTION_TREE_TIMEZONE",
        "notion.tree.timezone",
        "Asia/Shanghai",
    )?;
    week_key_and_title(Utc::now(), &timezone).map_err(BackendError::Validation)?;
    let mut category_growth_limit = resolve_i64_config(
        conn.as_ref(),
        "NOTION_TREE_CATEGORY_GROWTH_LIMIT",
        "notion.tree.category_growth_limit",
        32,
    )?;
    if resolve_env_bool("NOTION_TREE_FIXED_FIRST_LEVEL", true) {
        category_growth_limit = 0;
    }
    if category_growth_limit < 0 {
        return Err(BackendError::Validation(
            "NOTION_TREE_CATEGORY_GROWTH_LIMIT must be >= 0".to_string(),
        ));
    }

    Ok(NotionSyncConfigSnapshot {
        sync_mode,
        notion_token_configured: env_is_configured("NOTION_TOKEN"),
        notion_database_id_configured: env_is_configured("NOTION_DATABASE_ID"),
        notion_root_page_id_configured: env_is_configured("NOTION_ROOT_PAGE_ID"),
        timezone,
        category_growth_limit,
    })
}

// ── Analytics helpers (used only within this module) ─────────────────────────

fn normalize_month_key(raw: Option<String>) -> Result<String> {
    let fallback = Local::now().format("%Y-%m").to_string();
    let Some(candidate) = raw.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) else {
        return Ok(fallback);
    };
    let valid = candidate.len() == 7
        && candidate.chars().nth(4) == Some('-')
        && candidate.chars().enumerate().all(|(idx, ch)| {
            if idx == 4 {
                ch == '-'
            } else {
                ch.is_ascii_digit()
            }
        });
    if !valid {
        return Err(BackendError::Validation(
            "month must be in YYYY-MM format".to_string(),
        ));
    }
    Ok(candidate)
}

fn read_monthly_budget_limit_cny() -> f64 {
    env_with_shell_fallback("B2_G2_MONTHLY_BUDGET_CNY")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(100.0)
}

// ── Configuration readers (pub(super) so submodules can use them) ─────────────

fn env_is_configured(key: &str) -> bool {
    config::env_is_configured(key)
}

fn resolve_env_bool(key: &str, default_value: bool) -> bool {
    config::env_bool(key, default_value)
}

fn try_open_app_config_conn() -> Option<rusqlite::Connection> {
    let db_path = env_with_shell_fallback("HELPER_DB_PATH")?;
    let db_key = env_with_shell_fallback("HELPER_DB_KEY")?;
    let conn = match open_sqlcipher(&DbConfig {
        path: db_path,
        key: db_key,
    }) {
        Ok(conn) => conn,
        Err(e) => {
            warn!(error = %e, "open sqlcipher for config snapshot failed");
            return None;
        }
    };
    if let Err(e) = run_migrations(&conn) {
        warn!(error = %e, "run migrations for config snapshot failed");
        return None;
    }
    Some(conn)
}

pub(super) fn read_app_config_i64(conn: &rusqlite::Connection, key: &str) -> Result<Option<i64>> {
    config::read_app_config_i64(conn, key)
}

pub(super) fn read_notion_sync_mode(conn: Option<&rusqlite::Connection>) -> Result<String> {
    config::read_notion_sync_mode(conn)
}

fn resolve_string_config(
    conn: Option<&rusqlite::Connection>,
    env_key: &str,
    app_config_key: &str,
    default: &str,
) -> Result<String> {
    config::resolve_string_config(conn, env_key, app_config_key, default)
}

fn resolve_i64_config(
    conn: Option<&rusqlite::Connection>,
    env_key: &str,
    app_config_key: &str,
    default: i64,
) -> Result<i64> {
    config::resolve_i64_config(conn, env_key, app_config_key, default)
}

pub(super) fn required_env(key: &str) -> Result<String> {
    config::required_env(key)
}

pub(super) fn sanitize_limit(input: usize) -> usize {
    let clamped = input.clamp(1, 200);
    if clamped != input {
        warn!(input, clamped, "notion sync limit clamped");
    }
    clamped
}

pub(super) fn sanitize_sleep_ms(input: i64) -> u64 {
    let clamped = input.clamp(0, 5_000);
    if clamped != input {
        warn!(input, clamped, "notion sync sleep_ms clamped");
    }
    clamped as u64
}

fn record_notion_sync_job_run(
    conn: &rusqlite::Connection,
    started_at: &str,
    limit: usize,
    sleep_ms: u64,
    summary: Option<&SyncRunSummary>,
    run_error: Option<&BackendError>,
) -> Result<String> {
    let finished_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let status = if run_error.is_some() {
        "failed"
    } else if let Some(s) = summary {
        if s.attempted == 0 {
            "skipped"
        } else if s.failed > 0 {
            "partial"
        } else {
            "success"
        }
    } else {
        "failed"
    };
    let success_count = summary.map(|s| s.succeeded as i64).unwrap_or(0);
    let fail_count = if run_error.is_some() {
        1
    } else {
        summary.map(|s| s.failed as i64).unwrap_or(0)
    };
    let error_code = run_error.map(|e| e.code().as_str().to_string());
    let error_message = run_error.map(|e| e.to_string());
    let metadata_json = serde_json::json!({
        "limit": limit,
        "sleep_ms": sleep_ms,
        "summary": summary
    })
    .to_string();
    let id = format!("jr_nsync_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count,
            metadata_json, error_code, error_message, created_at
         ) VALUES (?1, 'notion_sync_once', 'notion', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?4)",
        rusqlite::params![
            &id,
            status,
            started_at,
            &finished_at,
            success_count,
            fail_count,
            metadata_json,
            error_code,
            error_message
        ],
    )?;
    Ok(id)
}

fn command_available(cmd: &str, args: &[&str]) -> Result<bool> {
    let mut candidates = vec![cmd];
    if cmd == "pdftotext" {
        candidates.extend(["/opt/homebrew/bin/pdftotext", "/usr/local/bin/pdftotext"]);
    } else if cmd == "textutil" {
        candidates.push("/usr/bin/textutil");
    }

    for candidate in candidates {
        match Command::new(candidate).args(args).output() {
            Ok(out) => {
                return Ok(out.status.success()
                    || !out.stdout.is_empty()
                    || !out.stderr.is_empty());
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                continue;
            }
            Err(e) => {
                return Err(BackendError::Internal(format!(
                    "local tool probe failed for {candidate}: {e}"
                )));
            }
        }
    }

    Ok(false)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{
        get_notion_sync_config_snapshot, record_notion_sync_job_run, sanitize_limit,
        sanitize_sleep_ms, NotionSyncConfigSnapshot, SyncRunSummary,
    };
    use crate::storage::db::run_migrations;
    use crate::BackendError;
    use rusqlite::Connection;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn sanitize_limit_clamps_range() {
        assert_eq!(sanitize_limit(0), 1);
        assert_eq!(sanitize_limit(10), 10);
        assert_eq!(sanitize_limit(999), 200);
    }

    #[test]
    fn sanitize_sleep_ms_clamps_range() {
        assert_eq!(sanitize_sleep_ms(-1), 0);
        assert_eq!(sanitize_sleep_ms(200), 200);
        assert_eq!(sanitize_sleep_ms(20_000), 5_000);
    }

    #[test]
    fn local_tool_status_is_queryable() {
        let status = super::get_local_tool_status().expect("local tool status");
        let _ = status.textutil;
        let _ = status.pdftotext;
    }

    #[test]
    fn record_notion_sync_job_run_marks_partial() {
        let conn = Connection::open_in_memory().expect("open memory");
        run_migrations(&conn).expect("migrations");

        let id = record_notion_sync_job_run(
            &conn,
            "2026-02-26 00:00:00",
            10,
            400,
            Some(&SyncRunSummary {
                scanned: 2,
                attempted: 2,
                succeeded: 1,
                failed: 1,
                requeued: 0,
                dead_lettered: 1,
            }),
            None,
        )
        .expect("record job_run");

        let (status, success_count, fail_count): (String, i64, i64) = conn
            .query_row(
                "SELECT status, success_count, fail_count FROM job_runs WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query job_run");
        assert_eq!(status, "partial");
        assert_eq!(success_count, 1);
        assert_eq!(fail_count, 1);
    }

    #[test]
    fn record_notion_sync_job_run_marks_failed_on_error() {
        let conn = Connection::open_in_memory().expect("open memory");
        run_migrations(&conn).expect("migrations");

        let id = record_notion_sync_job_run(
            &conn,
            "2026-02-26 00:00:00",
            10,
            400,
            None,
            Some(&BackendError::Internal("boom".to_string())),
        )
        .expect("record failed job_run");

        let (status, fail_count, error_code): (String, i64, Option<String>) = conn
            .query_row(
                "SELECT status, fail_count, error_code FROM job_runs WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query failed job_run");
        assert_eq!(status, "failed");
        assert_eq!(fail_count, 1);
        assert_eq!(error_code.as_deref(), Some("INT-9000"));
    }

    #[test]
    fn notion_sync_config_snapshot_reads_env_priority() {
        let _guard = env_lock().lock().expect("lock");
        std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
        std::env::set_var("NOTION_SYNC_MODE", "page_tree");
        std::env::set_var("NOTION_TREE_TIMEZONE", "UTC");
        std::env::set_var("NOTION_TREE_FIXED_FIRST_LEVEL", "0");
        std::env::set_var("NOTION_TREE_CATEGORY_GROWTH_LIMIT", "7");
        std::env::set_var("NOTION_TOKEN", "token");
        std::env::set_var("NOTION_DATABASE_ID", "db");
        std::env::set_var("NOTION_ROOT_PAGE_ID", "root");

        let snapshot: NotionSyncConfigSnapshot =
            get_notion_sync_config_snapshot().expect("config snapshot");
        assert_eq!(snapshot.sync_mode, "page_tree");
        assert_eq!(snapshot.timezone, "UTC");
        assert_eq!(snapshot.category_growth_limit, 7);
        assert!(snapshot.notion_token_configured);
        assert!(snapshot.notion_database_id_configured);
        assert!(snapshot.notion_root_page_id_configured);

        std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
        std::env::remove_var("NOTION_SYNC_MODE");
        std::env::remove_var("NOTION_TREE_TIMEZONE");
        std::env::remove_var("NOTION_TREE_FIXED_FIRST_LEVEL");
        std::env::remove_var("NOTION_TREE_CATEGORY_GROWTH_LIMIT");
        std::env::remove_var("NOTION_TOKEN");
        std::env::remove_var("NOTION_DATABASE_ID");
        std::env::remove_var("NOTION_ROOT_PAGE_ID");
    }

    #[test]
    fn notion_sync_config_snapshot_rejects_invalid_timezone() {
        let _guard = env_lock().lock().expect("lock");
        std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
        std::env::set_var("NOTION_TREE_FIXED_FIRST_LEVEL", "0");
        std::env::set_var("NOTION_TREE_TIMEZONE", "Mars/Olympus");

        let err = get_notion_sync_config_snapshot().expect_err("invalid tz");
        assert!(err.to_string().contains("invalid timezone"));

        std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
        std::env::remove_var("NOTION_TREE_FIXED_FIRST_LEVEL");
        std::env::remove_var("NOTION_TREE_TIMEZONE");
    }

    #[test]
    fn notion_sync_config_snapshot_fixed_first_level_forces_zero_growth_limit() {
        let _guard = env_lock().lock().expect("lock");
        std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
        std::env::set_var("NOTION_TREE_FIXED_FIRST_LEVEL", "1");
        std::env::set_var("NOTION_TREE_CATEGORY_GROWTH_LIMIT", "99");

        let snapshot: NotionSyncConfigSnapshot =
            get_notion_sync_config_snapshot().expect("config snapshot");
        assert_eq!(snapshot.category_growth_limit, 0);

        std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
        std::env::remove_var("NOTION_TREE_FIXED_FIRST_LEVEL");
        std::env::remove_var("NOTION_TREE_CATEGORY_GROWTH_LIMIT");
    }
}
