use futures::executor::block_on;
use helper_backend::app_core::AppCore;
use helper_backend::ipc::commands::run_notion_sync_once;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn run_notion_sync_once_requires_envs() {
    let _guard = env_lock().lock().expect("lock");
    std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");
    std::env::remove_var("NOTION_TOKEN");
    std::env::remove_var("NOTION_DATABASE_ID");

    let app = AppCore::default();
    let err = block_on(run_notion_sync_once(&app, Some(10))).expect_err("should fail");
    assert!(err.to_string().contains("HELPER_DB_PATH is required"));
    std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
}

fn test_db_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("helper_notion_env_{ts}.db"))
}

#[test]
fn run_notion_sync_once_page_tree_requires_root_page_id() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = test_db_path();
    std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
    std::env::set_var("HELPER_DB_PATH", db_path.to_string_lossy().as_ref());
    std::env::set_var("HELPER_DB_KEY", "unit-test-key");
    std::env::set_var("NOTION_TOKEN", "unit-test-token");
    std::env::set_var("NOTION_SYNC_MODE", "page_tree");
    std::env::remove_var("NOTION_DATABASE_ID");
    std::env::remove_var("NOTION_ROOT_PAGE_ID");

    let app = AppCore::default();
    let err = block_on(run_notion_sync_once(&app, Some(1))).expect_err("should fail");
    assert!(err.to_string().contains("NOTION_ROOT_PAGE_ID is required"));

    std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");
    std::env::remove_var("NOTION_TOKEN");
    std::env::remove_var("NOTION_SYNC_MODE");
    let _ = std::fs::remove_file(db_path);
}
