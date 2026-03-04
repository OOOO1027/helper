//! Tauri IPC facade: thin wrappers around core command functions that convert
//! `Result<T>` into `ApiResponse<T>` and register the Tauri command handlers.

use std::sync::{Arc, Mutex};
use uuid::Uuid;

use super::*;

fn ok<T>(data: T) -> ApiResponse<T> {
    ApiResponse {
        ok: true,
        request_id: Uuid::new_v4().to_string(),
        data: Some(data),
        error: None,
    }
}

fn err<T>(code: &str, msg: String, retryable: bool) -> ApiResponse<T> {
    ApiResponse {
        ok: false,
        request_id: Uuid::new_v4().to_string(),
        data: None,
        error: Some(ApiError {
            code: code.to_string(),
            message: msg,
            retryable,
        }),
    }
}

fn from_backend_error<T>(e: BackendError) -> ApiResponse<T> {
    err(e.code().as_str(), e.to_string(), e.retryable())
}

#[tauri::command]
pub fn run_daily_job(state: tauri::State<'_, Arc<Mutex<AppCore>>>) -> ApiResponse<JobRunResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::run_daily_job(&app) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn import_wechat_files(
    req: ImportWechatFilesReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<ImportSummary> {
    let app = state.lock().expect("app mutex poisoned");
    match super::import_wechat_files(&app, req.paths) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn collect(
    req: CollectReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<CollectResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::collect(&app, req.source, req.since, req.until) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn ingest_xhs_incremental(
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<CollectResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::ingest_xhs_incremental(&app) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_dashboard_metrics(
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<DashboardMetrics> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_dashboard_metrics(&app) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_collection_insight(
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<CollectionInsight> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_collection_insight(&app) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_review_items(
    req: GetReviewItemsReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<Paged<ReviewItem>> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_review_items(&app, req.filter, req.page) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_ingest_queue(
    req: GetIngestQueueReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<Paged<IngestQueueItem>> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_ingest_queue(&app, req.page) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_review_queue(
    req: GetReviewQueueReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<Paged<ReviewItem>> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_review_queue(&app, req.filter, req.page) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn update_review_item(
    req: UpdateReviewItemReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<ReviewItem> {
    let app = state.lock().expect("app mutex poisoned");
    match super::update_review_item(&app, req.id, req.patch) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn retry_failed_items(
    req: RetryFailedItemsReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<RetryResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::retry_failed_items(&app, req.ids) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn retry_dead_letters(
    req: RetryFailedItemsReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<RetryResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::retry_dead_letters(&app, req.ids) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_budget_status(
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<BudgetStatus> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_budget_status(&app) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_ai_usage_monthly_summary(
    req: GetAiUsageMonthlyReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<AiUsageMonthlySummary> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_ai_usage_monthly_summary(&app, req.month) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_sync_logs(
    req: GetSyncLogsReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<Paged<SyncLog>> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_sync_logs(&app, req.range, req.page) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_sync_daily_stats(
    req: GetSyncDailyStatsReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<Paged<SyncDailyStat>> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_sync_daily_stats(&app, req.range, req.page, req.job_type, req.status) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn run_pipeline_preview(
    req: PipelinePreviewReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<PipelinePreviewResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::run_pipeline_preview(&app, req) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn run_pipeline_persist(
    req: PipelinePreviewReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<PipelineExecuteResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::run_pipeline_persist(&app, req) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn import_wechat_and_persist(
    req: ImportWechatFilesReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<ImportPersistSummary> {
    let app = state.lock().expect("app mutex poisoned");
    match super::import_wechat_and_persist(&app, req.paths) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn run_notion_sync_once(
    req: RunNotionSyncReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<SyncRunSummary> {
    let app = state.lock().expect("app mutex poisoned").clone();
    match tauri::async_runtime::block_on(super::run_notion_sync_once(&app, req.limit)) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn publish_approved_to_notion(
    req: PublishApprovedReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<PublishResult> {
    let app = state.lock().expect("app mutex poisoned").clone();
    match tauri::async_runtime::block_on(super::publish_approved_to_notion(&app, req.limit)) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_publish_queue(
    req: GetPublishQueueReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<Paged<PublishTaskItem>> {
    let app = state.lock().expect("app mutex poisoned");
    match super::get_publish_queue(&app, req.page) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_publish_history(
    req: GetPublishHistoryReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<Paged<PublishHistoryItem>> {
    let app = state.lock().expect("app mutex poisoned");
    let filters = PublishHistoryFilters {
        state: req.state,
        sync_mode: req.sync_mode,
        retryable: req.retryable,
    };
    match super::get_publish_history_with_filters(&app, req.range, req.page, filters) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn mark_source_inactive(
    req: MarkSourceInactiveReq,
    state: tauri::State<'_, Arc<Mutex<AppCore>>>,
) -> ApiResponse<SourceStateMarkResult> {
    let app = state.lock().expect("app mutex poisoned");
    match super::mark_source_inactive(&app, req.item_id) {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_local_tool_status() -> ApiResponse<LocalToolStatus> {
    match super::get_local_tool_status() {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}

#[tauri::command]
pub fn get_notion_sync_config_snapshot() -> ApiResponse<NotionSyncConfigSnapshot> {
    match super::get_notion_sync_config_snapshot() {
        Ok(data) => ok(data),
        Err(e) => from_backend_error(e),
    }
}
