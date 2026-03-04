use helper_backend::app_core::{AppCore, DateRange, PageReq};
use helper_backend::ipc::commands::get_sync_logs;
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn get_sync_logs_requires_db_env() {
    let _guard = env_lock().lock().expect("lock");
    std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");

    let app = AppCore::default();
    let err = get_sync_logs(
        &app,
        DateRange {
            from: "2026-02-01 00:00:00".to_string(),
            to: "2026-02-28 23:59:59".to_string(),
        },
        PageReq {
            page: 1,
            page_size: 20,
        },
    )
    .expect_err("should fail");
    assert!(err.to_string().contains("HELPER_DB_PATH is required"));
    std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
}
