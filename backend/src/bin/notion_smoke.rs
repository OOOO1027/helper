use helper_backend::env_runtime::env_with_shell_fallback;
use helper_backend::storage::db::{open_sqlcipher, run_migrations, DbConfig};
use helper_backend::sync_notion::notion_api::NotionHttpClient;
use helper_backend::sync_notion::smoke::{
    persist_smoke_report, run_notion_page_tree_smoke_with_conn, run_notion_sync_smoke_with_conn,
};
use helper_backend::{BackendError, Result};
use tracing::{info, warn};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "helper_backend=info".to_string()),
        )
        .init();

    if let Err(err) = run() {
        eprintln!(
            "{{\"ok\":false,\"code\":\"{}\",\"message\":\"{}\"}}",
            err.code().as_str(),
            err
        );
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let db_path = required_env("HELPER_DB_PATH")?;
    let db_key = required_env("HELPER_DB_KEY")?;
    let notion_token = required_env("NOTION_TOKEN")?;

    let limit = parse_usize_env("NOTION_SMOKE_LIMIT", 20, 1, 200);
    let check_top = parse_usize_env("NOTION_SMOKE_CHECK_TOP", 5, 1, 50);
    let sleep_ms = parse_u64_env("NOTION_SMOKE_SLEEP_MS", 400, 0, 5_000);

    let conn = open_sqlcipher(&DbConfig {
        path: db_path,
        key: db_key,
    })?;
    run_migrations(&conn)?;

    let mode = read_notion_sync_mode(&conn)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| BackendError::Internal(format!("build tokio runtime failed: {e}")))?;

    let report = match mode.as_str() {
        "database" => {
            let notion_database_id = required_env("NOTION_DATABASE_ID")?;
            let client = NotionHttpClient::new(notion_token.clone(), notion_database_id)?;
            runtime.block_on(run_notion_sync_smoke_with_conn(
                &conn, &client, limit, check_top, sleep_ms,
            ))?
        }
        "page_tree" => {
            let notion_root_page_id = required_env("NOTION_ROOT_PAGE_ID")?;
            let client = NotionHttpClient::new_page_tree(notion_token.clone())?;
            runtime.block_on(run_notion_page_tree_smoke_with_conn(
                &conn,
                &client,
                &notion_root_page_id,
                limit,
                check_top,
                sleep_ms,
            ))?
        }
        _ => {
            return Err(BackendError::Validation(format!(
                "NOTION_SYNC_MODE must be 'page_tree' or 'database', got '{}'",
                mode
            )))
        }
    };
    let job_run_id = persist_smoke_report(&conn, &report)?;

    info!(
        job_run_id = %job_run_id,
        attempted = report.summary.attempted,
        succeeded = report.summary.succeeded,
        failed = report.summary.failed,
        checked = report.checked,
        mismatched = report.mismatched,
        mode = %report.mode,
        "notion smoke completed"
    );
    let output = serde_json::json!({
        "job_run_id": job_run_id,
        "report": report
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|e| BackendError::Internal(format!("encode smoke report failed: {e}")))?
    );

    if report.summary.attempted == 0 {
        warn!("notion smoke skipped: no pending sync records");
        std::process::exit(3);
    }
    if report.mismatched > 0 {
        warn!(
            mismatched = report.mismatched,
            "notion smoke found mismatches"
        );
        std::process::exit(2);
    }

    Ok(())
}

fn read_notion_sync_mode(conn: &rusqlite::Connection) -> Result<String> {
    if let Some(mode) = env_with_shell_fallback("NOTION_SYNC_MODE") {
        let normalized = mode.trim().to_lowercase();
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }
    let db_mode: Option<String> = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'notion.sync.mode'",
            [],
            |row| row.get(0),
        )
        .ok();
    if let Some(mode) = db_mode {
        let normalized = mode.trim().to_lowercase();
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }
    Ok("page_tree".to_string())
}

fn required_env(key: &str) -> Result<String> {
    if key == "NOTION_TOKEN" {
        return std::env::var(key)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                BackendError::Validation(
                    "NOTION_TOKEN is required in current process env".to_string(),
                )
            });
    }
    env_with_shell_fallback(key)
        .ok_or_else(|| BackendError::Validation(format!("{key} is required")))
}

fn parse_usize_env(key: &str, default: usize, min: usize, max: usize) -> usize {
    match std::env::var(key) {
        Ok(raw) => match raw.parse::<usize>() {
            Ok(v) => v.clamp(min, max),
            Err(_) => {
                warn!(key, value = %raw, fallback = default, "invalid usize env, using fallback");
                default
            }
        },
        Err(_) => default,
    }
}

fn parse_u64_env(key: &str, default: u64, min: u64, max: u64) -> u64 {
    match std::env::var(key) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(v) => v.clamp(min, max),
            Err(_) => {
                warn!(key, value = %raw, fallback = default, "invalid u64 env, using fallback");
                default
            }
        },
        Err(_) => default,
    }
}
