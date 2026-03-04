#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use helper_backend::app_core::AppCore;
use helper_backend::env_runtime::bootstrap_env_from_shell;
use helper_backend::ipc::commands::tauri_api;

fn main() {
    bootstrap_env_from_shell(&[
        "HELPER_DB_PATH",
        "HELPER_DB_KEY",
        "NOTION_TOKEN",
        "NOTION_DATABASE_ID",
        "NOTION_ROOT_PAGE_ID",
        "NOTION_TREE_FIXED_FIRST_LEVEL",
        "XHS_COOKIE",
        "XHS_COOKIE_FILE",
        "QWEN_API_KEY",
        "QWEN_MODEL",
        "B2_G2_AI_ENABLED",
        "B2_G2_XHS_DEEP_FETCH",
        "B2_G2_QWEN_MAX_CALLS_PER_RUN",
        "B2_G2_XHS_DEEP_FETCH_TIMEOUT_MS",
        "B2_G2_MONTHLY_BUDGET_CNY",
    ]);
    let state = Arc::new(Mutex::new(AppCore::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Set default DB path to the app's data directory if not already configured.
            // This ensures the app works out-of-the-box without manual env var setup.
            if std::env::var("HELPER_DB_PATH").is_err() {
                if let Ok(data_dir) = app.path().app_data_dir() {
                    std::fs::create_dir_all(&data_dir).ok();
                    let db_path = data_dir.join("helper.db");
                    std::env::set_var("HELPER_DB_PATH", db_path.to_string_lossy().as_ref());
                }
            }
            Ok(())
        })
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            tauri_api::run_daily_job,
            tauri_api::import_wechat_files,
            tauri_api::import_wechat_and_persist,
            tauri_api::collect,
            tauri_api::ingest_xhs_incremental,
            tauri_api::get_dashboard_metrics,
            tauri_api::get_collection_insight,
            tauri_api::get_ingest_queue,
            tauri_api::get_review_items,
            tauri_api::get_review_queue,
            tauri_api::update_review_item,
            tauri_api::retry_failed_items,
            tauri_api::retry_dead_letters,
            tauri_api::get_budget_status,
            tauri_api::get_ai_usage_monthly_summary,
            tauri_api::get_sync_logs,
            tauri_api::get_sync_daily_stats,
            tauri_api::run_notion_sync_once,
            tauri_api::publish_approved_to_notion,
            tauri_api::get_publish_queue,
            tauri_api::get_publish_history,
            tauri_api::mark_source_inactive,
            tauri_api::get_local_tool_status,
            tauri_api::get_notion_sync_config_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running helper desktop");
}
