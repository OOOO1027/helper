use helper_backend::app_core::AppCore;
use helper_backend::ipc::commands::retry_dead_letters;
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn retry_dead_letters_requires_db_env() {
    let _guard = env_lock().lock().expect("lock");
    std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");

    let app = AppCore::default();
    let err = retry_dead_letters(&app, vec!["dlq_a".to_string()]).expect_err("should fail");
    assert!(err.to_string().contains("HELPER_DB_PATH is required"));
    std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
}
