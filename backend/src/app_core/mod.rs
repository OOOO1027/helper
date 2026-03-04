use chrono::Local;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use self::adapters::sqlite_core_port::SqliteCoreDataPort;
use self::ports::CoreDataPort;
use crate::budget_guard::BudgetStatus;
use crate::env_runtime::env_with_shell_fallback;
use crate::storage::db::{open_sqlcipher, run_migrations, DbConfig};
use crate::{BackendError, Result};

pub mod adapters;
pub mod ports;

mod dashboard;
mod dead_letter_recovery;
mod pipeline;
mod review_queue;
mod sync_analytics;
mod wechat_import;
mod xhs_collection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRunResult {
    pub job_run_id: String,
    pub status: String,
    pub queued_tasks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub batch_id: String,
    pub total: u32,
    pub imported: u32,
    pub duplicates: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPersistSummary {
    pub batch_id: String,
    pub total: u32,
    pub parsed_ok: u32,
    pub duplicates: u32,
    pub parse_failed: u32,
    pub persisted: u32,
    pub persist_failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectResult {
    pub source: String,
    pub fetched: u32,
    pub stored: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub today_collected: u32,
    pub pending_review: u32,
    pub classification_accuracy: f64,
    pub summary_usability: f64,
    pub budget_usage_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSourceBreakdown {
    pub source: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRecentItem {
    pub source: String,
    pub title: String,
    pub collected_at: String,
    pub review_state: String,
    pub sync_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInsight {
    pub day: String,
    pub today_total: u32,
    pub sources: Vec<CollectionSourceBreakdown>,
    pub review_pending: u32,
    pub review_done: u32,
    pub review_rejected: u32,
    pub direct_no_review: u32,
    pub sync_pending: u32,
    pub sync_success: u32,
    pub sync_failed: u32,
    pub sync_retry: u32,
    pub sync_not_started: u32,
    pub recent_items: Vec<CollectionRecentItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: String,
    pub normalized_item_id: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub priority: f64,
    pub quality_band: String,
    pub reason: String,
    pub state: String,
    pub publish_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFilter {
    pub state: Option<String>,
    pub min_priority: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageReq {
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewPatch {
    pub state: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryResult {
    pub requested: u32,
    pub requeued: u32,
    pub ignored: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLog {
    pub id: String,
    pub state: String,
    pub error_code: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDailyStat {
    pub day: String,
    pub job_type: String,
    pub run_count: u32,
    pub success_runs: u32,
    pub partial_runs: u32,
    pub failed_runs: u32,
    pub success_rate: f64,
    pub avg_success_count: f64,
    pub avg_fail_count: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelinePreviewReq {
    pub source: String,
    pub source_url: Option<String>,
    pub published_at: Option<String>,
    pub content_raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelinePreviewResult {
    pub dedupe_key: String,
    pub labels: Vec<String>,
    pub summary: String,
    pub classification_confidence: f64,
    pub summary_confidence: f64,
    pub review_required: bool,
    pub gate: String,
    pub notion_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineExecuteResult {
    pub source_item_id: String,
    pub normalized_item_id: String,
    pub analysis_result_id: String,
    pub review_queued: bool,
    pub sync_record_id: String,
    pub notion_action: String,
}

#[derive(Debug, Clone)]
pub struct AppCore {
    pub budget: BudgetStatus,
}

impl Default for AppCore {
    fn default() -> Self {
        Self {
            budget: BudgetStatus {
                month: Local::now().format("%Y-%m").to_string(),
                limit_cny: 100.0,
                used_cny: 0.0,
            },
        }
    }
}

fn normalize_optional_filter(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_status_filter(status: Option<String>) -> Result<Option<String>> {
    let normalized = normalize_optional_filter(status).map(|s| s.to_ascii_lowercase());
    if let Some(ref value) = normalized {
        if !matches!(
            value.as_str(),
            "started" | "success" | "failed" | "partial" | "skipped"
        ) {
            return Err(BackendError::Validation(format!(
                "status must be one of started|success|failed|partial|skipped, got {value}"
            )));
        }
    }
    Ok(normalized)
}

fn normalize_review_state_filter(state: Option<String>) -> Result<Option<String>> {
    let normalized = normalize_optional_filter(state).map(|s| s.to_ascii_lowercase());
    if let Some(ref value) = normalized {
        if !matches!(
            value.as_str(),
            "pending" | "processing" | "done" | "rejected"
        ) {
            return Err(BackendError::Validation(format!(
                "state must be one of pending|processing|done|rejected, got {value}"
            )));
        }
    }
    Ok(normalized)
}

fn open_conn_from_env() -> Result<Connection> {
    let db_path = env_with_shell_fallback("HELPER_DB_PATH")
        .ok_or_else(|| BackendError::Validation("HELPER_DB_PATH is required".to_string()))?;
    let db_key = env_with_shell_fallback("HELPER_DB_KEY")
        .ok_or_else(|| BackendError::Validation("HELPER_DB_KEY is required".to_string()))?;
    let conn = open_sqlcipher(&DbConfig {
        path: db_path,
        key: db_key,
    })?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn stable_hash(input: &str) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in input.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

fn escape_json(v: &str) -> String {
    v.replace('\\', "\\\\").replace('"', "\\\"")
}

impl AppCore {
    pub fn run_daily_job(&self) -> Result<JobRunResult> {
        let queued_tasks = if self.budget.daily_report_enabled() {
            4
        } else {
            3
        };
        info!(queued_tasks, "daily job planned");
        Ok(JobRunResult {
            job_run_id: Uuid::new_v4().to_string(),
            status: "queued".to_string(),
            queued_tasks,
        })
    }

    pub fn collect(
        &self,
        source: &str,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<CollectResult> {
        let mut conn = open_conn_from_env()?;
        self.collect_with_conn(&mut conn, source, since, until)
    }

    pub fn get_dashboard_metrics(&self) -> Result<DashboardMetrics> {
        let conn = open_conn_from_env()?;
        self.get_dashboard_metrics_with_conn(&conn)
    }

    pub fn get_collection_insight(&self) -> Result<CollectionInsight> {
        let conn = open_conn_from_env()?;
        self.get_collection_insight_with_conn(&conn)
    }

    pub fn get_review_items(
        &self,
        filter: ReviewFilter,
        page: PageReq,
    ) -> Result<Paged<ReviewItem>> {
        let conn = open_conn_from_env()?;
        self.get_review_items_with_conn(&conn, filter, page)
    }

    pub fn update_review_item(&self, id: &str, patch: ReviewPatch) -> Result<ReviewItem> {
        let conn = open_conn_from_env()?;
        self.update_review_item_with_conn(&conn, id, patch)
    }

    pub fn retry_failed_items(&self, ids: &[String]) -> Result<RetryResult> {
        let conn = open_conn_from_env()?;
        self.retry_failed_items_with_conn(&conn, ids)
    }

    pub fn collect_with_conn(
        &self,
        conn: &mut Connection,
        source: &str,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<CollectResult> {
        if !matches!(source, "xhs" | "wechat" | "bili") {
            return Err(BackendError::Validation(format!(
                "unsupported source: {source}"
            )));
        }
        if source == "xhs" {
            return self.collect_xhs_with_conn(conn, since, until);
        }
        let port = SqliteCoreDataPort::new(conn);
        let (fetched, stored) = port.collect_counts(source, since, until)?;
        info!(source, fetched, stored, "collect requested");
        Ok(CollectResult {
            source: source.to_string(),
            fetched,
            stored,
        })
    }

    pub fn get_budget_status(&self) -> Result<BudgetStatus> {
        Ok(self.budget.clone())
    }
}
