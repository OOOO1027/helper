use helper_backend::app_core::{AppCore, PipelinePreviewReq};
use helper_backend::ipc::commands::run_pipeline_persist;
use std::fs;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn temp_db_path() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let mut p = std::env::temp_dir();
    p.push(format!("helper_backend_ipc_{ts}.db"));
    p.to_string_lossy().to_string()
}

#[test]
fn run_pipeline_persist_requires_env() {
    let _guard = env_lock().lock().expect("lock");
    std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");

    let app = AppCore::default();
    let req = PipelinePreviewReq {
        source: "wechat".to_string(),
        source_url: Some("https://example.com/path?id=9".to_string()),
        published_at: Some("2026-02-25T21:00:00+08:00".to_string()),
        content_raw: "env test".to_string(),
    };
    let err = run_pipeline_persist(&app, req).expect_err("should fail without env");
    assert!(err.to_string().contains("HELPER_DB_PATH is required"));
    std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
}

#[test]
fn run_pipeline_persist_is_idempotent_across_calls() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    std::env::set_var("HELPER_DB_PATH", &db_path);
    std::env::set_var("HELPER_DB_KEY", "unit-test-key");

    let app = AppCore::default();
    let req = PipelinePreviewReq {
        source: "wechat".to_string(),
        source_url: Some("https://example.com/path?id=10".to_string()),
        published_at: Some("2026-02-25T21:00:00+08:00".to_string()),
        content_raw: "persist test".to_string(),
    };
    let first = run_pipeline_persist(&app, req.clone()).expect("first run");
    let second = run_pipeline_persist(&app, req).expect("second run");
    assert_eq!(first.normalized_item_id, second.normalized_item_id);
    assert_eq!(first.notion_action, "create");
    assert_eq!(second.notion_action, "update");

    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");
    let _ = fs::remove_file(db_path);
}
