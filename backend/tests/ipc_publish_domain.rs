use futures::executor::block_on;
use helper_backend::app_core::{AppCore, DateRange, PageReq};
use helper_backend::ipc::commands::{
    get_ai_usage_monthly_summary, get_publish_history, get_publish_history_with_filters,
    get_publish_queue, mark_source_inactive, publish_approved_to_notion, PublishHistoryFilters,
};
use helper_backend::storage::db::{open_sqlcipher, run_migrations, DbConfig};
use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn temp_db_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("helper_publish_ipc_{ts}.db"))
}

fn set_test_env(db_path: &Path) {
    std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
    std::env::set_var("HELPER_DB_PATH", db_path.to_string_lossy().as_ref());
    std::env::set_var("HELPER_DB_KEY", "unit-test-key");
    std::env::remove_var("NOTION_TOKEN");
    std::env::remove_var("NOTION_DATABASE_ID");
    std::env::remove_var("NOTION_ROOT_PAGE_ID");
    std::env::remove_var("NOTION_SYNC_MODE");
}

fn clear_test_env(db_path: &Path) {
    std::env::remove_var("HELPER_DISABLE_SHELL_ENV_FALLBACK");
    std::env::remove_var("HELPER_DB_PATH");
    std::env::remove_var("HELPER_DB_KEY");
    std::env::remove_var("NOTION_TOKEN");
    std::env::remove_var("NOTION_DATABASE_ID");
    std::env::remove_var("NOTION_ROOT_PAGE_ID");
    std::env::remove_var("NOTION_SYNC_MODE");
    let _ = fs::remove_file(db_path);
}

fn open_test_conn(db_path: &Path) -> Connection {
    let conn = open_sqlcipher(&DbConfig {
        path: db_path.to_string_lossy().to_string(),
        key: "unit-test-key".to_string(),
    })
    .expect("open sqlcipher");
    run_migrations(&conn).expect("migrations");
    conn
}

fn seed_xhs_sync_row(conn: &Connection, suffix: &str, sync_state: &str) {
    let source_id = format!("src_{suffix}");
    let normalized_id = format!("norm_{suffix}");
    let now = "2026-02-26 09:00:00";

    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status, collected_day
         ) VALUES (?1, 'xhs', ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'new', ?9)",
        params![
            source_id,
            format!("ext_{suffix}"),
            format!("https://example.com/{suffix}"),
            format!("uh_{suffix}"),
            format!("title_{suffix}"),
            format!("body_{suffix}"),
            now,
            now,
            "2026-02-26"
        ],
    )
    .expect("insert source item");

    conn.execute(
        "INSERT INTO normalized_items(
            id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint, quality_score, value_score, gate_status, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 80, 0.8, 'direct', ?7, ?7)",
        params![
            normalized_id,
            source_id,
            format!("https://example.com/{suffix}"),
            format!("cuh_{suffix}"),
            format!("clean_{suffix}"),
            format!("fp_{suffix}"),
            now
        ],
    )
    .expect("insert normalized item");

    conn.execute(
        "INSERT INTO sync_records(
            id, normalized_item_id, target, sync_state, retry_count, next_retry_at, created_at, updated_at
         ) VALUES (?1, ?2, 'notion', ?3, 0, NULL, ?4, ?4)",
        params![format!("sync_{suffix}"), normalized_id, sync_state, now],
    )
    .expect("insert sync record");
}

fn seed_pending_review(conn: &Connection, suffix: &str) {
    let normalized_id = format!("norm_{suffix}");
    let now = "2026-02-26 09:00:00";
    conn.execute(
        "INSERT INTO analysis_results(
            id, normalized_item_id, classification_json, summary_text,
            c_score, s_score, j_score, d_score, value_score, quality_score,
            review_required, review_status, final_status, created_at, updated_at
         ) VALUES (?1, ?2, '[\"学习\"]', 'summary', 0.8, 0.8, 0.8, 0.8, 0.8, 80, 1, 'pending', 'review', ?3, ?3)",
        params![format!("ana_{suffix}"), normalized_id, now],
    )
    .expect("insert analysis");

    conn.execute(
        "INSERT INTO review_queue(
            id, normalized_item_id, analysis_result_id, priority, reason, state, created_at, updated_at
         ) VALUES (?1, ?2, ?3, 0.7, 'manual review', 'pending', ?4, ?4)",
        params![
            format!("rq_{suffix}"),
            normalized_id,
            format!("ana_{suffix}"),
            now
        ],
    )
    .expect("insert review queue");
}

#[test]
fn get_publish_queue_seeds_and_maps_publish_states() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);
    seed_xhs_sync_row(&conn, "pending", "pending");
    seed_xhs_sync_row(&conn, "await", "pending");
    seed_pending_review(&conn, "await");
    drop(conn);

    let app = AppCore::default();
    let result = get_publish_queue(
        &app,
        PageReq {
            page: 1,
            page_size: 20,
        },
    )
    .expect("get publish queue");

    let pending = result
        .items
        .iter()
        .find(|item| item.item_id == "src_pending")
        .expect("pending task exists");
    assert_eq!(pending.state, "pending");

    let await_review = result
        .items
        .iter()
        .find(|item| item.item_id == "src_await")
        .expect("await-review task exists");
    assert_eq!(await_review.state, "ignored");
    assert_eq!(await_review.error_code.as_deref(), Some("await_review"));

    clear_test_env(&db_path);
}

#[test]
fn get_publish_history_prefers_publish_audit_grouping() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);

    conn.execute(
        "INSERT INTO publish_audit(
            id, publish_task_id, item_id, request_id, status, latency_ms, error_code, error_message, pipeline_stage, sync_mode, retryable, created_at
         ) VALUES ('pa_a', 'pt_a', 'src_a', 'req_a', 'success', 120, NULL, NULL, 'publish_approved_to_notion', 'page_tree', 0, '2026-02-26 08:10:00')",
        [],
    )
    .expect("insert pa_a");
    conn.execute(
        "INSERT INTO publish_audit(
            id, publish_task_id, item_id, request_id, status, latency_ms, error_code, error_message, pipeline_stage, sync_mode, retryable, created_at
         ) VALUES ('pa_b', 'pt_b', 'src_b', 'req_a', 'failed', 160, 'MOD-3002', 'failed', 'publish_approved_to_notion', 'page_tree', 1, '2026-02-26 08:11:00')",
        [],
    )
    .expect("insert pa_b");
    conn.execute(
        "INSERT INTO publish_audit(
            id, publish_task_id, item_id, request_id, status, latency_ms, error_code, error_message, pipeline_stage, sync_mode, retryable, created_at
         ) VALUES ('pa_c', 'pt_c', 'src_c', 'req_b', 'success', 100, NULL, NULL, 'publish_approved_to_notion', 'database', 0, '2026-02-26 08:20:00')",
        [],
    )
    .expect("insert pa_c");
    conn.execute(
        "INSERT INTO notion_page_refs(
            id, item_id, notion_page_id, category, week_key, route_reason, published_hash, created_at, updated_at
         ) VALUES
            ('nref_a', 'src_a', 'page_a', 'Inbox', '2026-W09', NULL, 'hash_a', '2026-02-26 08:10:01', '2026-02-26 08:10:01'),
            ('nref_b', 'src_b', 'page_b', 'Inbox', '2026-W09', 'growth_limit_exceeded', 'hash_b', '2026-02-26 08:11:01', '2026-02-26 08:11:01'),
            ('nref_c', 'src_c', 'page_c', '工作', '2026-W09', NULL, 'hash_c', '2026-02-26 08:20:01', '2026-02-26 08:20:01')",
        [],
    )
    .expect("insert notion_page_refs");
    drop(conn);

    let app = AppCore::default();
    let result = get_publish_history(
        &app,
        DateRange {
            from: "2026-02-26 00:00:00".to_string(),
            to: "2026-02-26 23:59:59".to_string(),
        },
        PageReq {
            page: 1,
            page_size: 20,
        },
    )
    .expect("get publish history");

    assert_eq!(result.total, 2);
    let req_a = result
        .items
        .iter()
        .find(|item| item.id == "req_a")
        .expect("req_a exists");
    assert_eq!(req_a.state, "partial");
    assert_eq!(req_a.success_count, 1);
    assert_eq!(req_a.fail_count, 1);
    assert_eq!(req_a.error_code.as_deref(), Some("MOD-3002"));
    assert_eq!(
        req_a.pipeline_stage.as_deref(),
        Some("publish_approved_to_notion")
    );
    assert_eq!(req_a.sync_mode.as_deref(), Some("page_tree"));
    assert_eq!(req_a.retryable, Some(true));
    assert_eq!(req_a.category.as_deref(), Some("Inbox"));
    assert_eq!(req_a.week_key.as_deref(), Some("2026-W09"));
    assert_eq!(req_a.route_reason.as_deref(), Some("growth_limit_exceeded"));

    let req_b = result
        .items
        .iter()
        .find(|item| item.id == "req_b")
        .expect("req_b exists");
    assert_eq!(req_b.state, "success");
    assert_eq!(req_b.success_count, 1);
    assert_eq!(req_b.fail_count, 0);
    assert_eq!(
        req_b.pipeline_stage.as_deref(),
        Some("publish_approved_to_notion")
    );
    assert_eq!(req_b.sync_mode.as_deref(), Some("database"));
    assert_eq!(req_b.retryable, Some(false));
    assert_eq!(req_b.category.as_deref(), Some("工作"));
    assert_eq!(req_b.week_key.as_deref(), Some("2026-W09"));
    assert!(req_b.route_reason.is_none());

    clear_test_env(&db_path);
}

#[test]
fn get_publish_history_falls_back_to_job_runs_when_no_publish_audit() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);

    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, error_code, error_message, created_at
         ) VALUES ('jr_a', 'notion_sync_once', 'notion', 'success', '2026-02-26 09:00:00', '2026-02-26 09:00:10', 3, 0, '{}', NULL, NULL, '2026-02-26 09:00:10')",
        [],
    )
    .expect("insert jr_a");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, error_code, error_message, created_at
         ) VALUES ('jr_b', 'notion_sync_once', 'notion', 'failed', '2026-02-26 10:00:00', '2026-02-26 10:00:12', 0, 1, '{}', 'HTTP-429', 'rate limited', '2026-02-26 10:00:12')",
        [],
    )
    .expect("insert jr_b");
    drop(conn);

    let app = AppCore::default();
    let result = get_publish_history(
        &app,
        DateRange {
            from: "2026-02-26 00:00:00".to_string(),
            to: "2026-02-26 23:59:59".to_string(),
        },
        PageReq {
            page: 1,
            page_size: 20,
        },
    )
    .expect("get fallback publish history");

    assert_eq!(result.total, 2);
    let jr_a = result
        .items
        .iter()
        .find(|item| item.id == "jr_a")
        .expect("jr_a exists");
    assert!(jr_a.pipeline_stage.is_none());
    assert!(jr_a.sync_mode.is_none());
    assert!(jr_a.retryable.is_none());
    assert!(jr_a.category.is_none());
    assert!(jr_a.week_key.is_none());
    assert!(jr_a.route_reason.is_none());

    let jr_b = result
        .items
        .iter()
        .find(|item| item.id == "jr_b")
        .expect("jr_b exists");
    assert!(jr_b.pipeline_stage.is_none());
    assert!(jr_b.sync_mode.is_none());
    assert!(jr_b.retryable.is_none());
    assert!(jr_b.category.is_none());
    assert!(jr_b.week_key.is_none());
    assert!(jr_b.route_reason.is_none());

    clear_test_env(&db_path);
}

#[test]
fn get_publish_history_publish_audit_filters_hit_expected_group() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);
    conn.execute(
        "INSERT INTO publish_audit(
            id, publish_task_id, item_id, request_id, status, latency_ms, error_code, error_message, pipeline_stage, sync_mode, retryable, created_at
         ) VALUES ('pa_fa', 'pt_fa', 'src_fa', 'req_filter_a', 'success', 100, NULL, NULL, 'publish_approved_to_notion', 'page_tree', 0, '2026-02-26 08:10:00')",
        [],
    )
    .expect("insert pa_fa");
    conn.execute(
        "INSERT INTO publish_audit(
            id, publish_task_id, item_id, request_id, status, latency_ms, error_code, error_message, pipeline_stage, sync_mode, retryable, created_at
         ) VALUES ('pa_fb', 'pt_fb', 'src_fb', 'req_filter_a', 'failed', 160, 'MOD-3002', 'failed', 'publish_approved_to_notion', 'page_tree', 1, '2026-02-26 08:11:00')",
        [],
    )
    .expect("insert pa_fb");
    conn.execute(
        "INSERT INTO publish_audit(
            id, publish_task_id, item_id, request_id, status, latency_ms, error_code, error_message, pipeline_stage, sync_mode, retryable, created_at
         ) VALUES ('pa_fc', 'pt_fc', 'src_fc', 'req_filter_b', 'success', 90, NULL, NULL, 'publish_approved_to_notion', 'database', 0, '2026-02-26 08:20:00')",
        [],
    )
    .expect("insert pa_fc");
    drop(conn);

    let app = AppCore::default();
    let result = get_publish_history_with_filters(
        &app,
        DateRange {
            from: "2026-02-26 00:00:00".to_string(),
            to: "2026-02-26 23:59:59".to_string(),
        },
        PageReq {
            page: 1,
            page_size: 20,
        },
        PublishHistoryFilters {
            state: Some("partial".to_string()),
            sync_mode: Some("page_tree".to_string()),
            retryable: Some(true),
        },
    )
    .expect("get filtered publish history");

    assert_eq!(result.total, 1);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, "req_filter_a");
    assert_eq!(result.items[0].state, "partial");
    assert_eq!(result.items[0].sync_mode.as_deref(), Some("page_tree"));
    assert_eq!(result.items[0].retryable, Some(true));

    clear_test_env(&db_path);
}

#[test]
fn get_publish_history_legacy_state_filter_only_and_sync_mode_returns_empty() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, error_code, error_message, created_at
         ) VALUES ('jr_l1', 'notion_sync_once', 'notion', 'success', '2026-02-26 09:00:00', '2026-02-26 09:00:10', 3, 0, '{}', NULL, NULL, '2026-02-26 09:00:10')",
        [],
    )
    .expect("insert jr_l1");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, error_code, error_message, created_at
         ) VALUES ('jr_l2', 'notion_sync_once', 'notion', 'failed', '2026-02-26 10:00:00', '2026-02-26 10:00:12', 0, 1, '{}', 'HTTP-429', 'rate limited', '2026-02-26 10:00:12')",
        [],
    )
    .expect("insert jr_l2");
    drop(conn);

    let app = AppCore::default();
    let state_only = get_publish_history_with_filters(
        &app,
        DateRange {
            from: "2026-02-26 00:00:00".to_string(),
            to: "2026-02-26 23:59:59".to_string(),
        },
        PageReq {
            page: 1,
            page_size: 20,
        },
        PublishHistoryFilters {
            state: Some("failed".to_string()),
            sync_mode: None,
            retryable: None,
        },
    )
    .expect("legacy state-only filter");
    assert_eq!(state_only.total, 1);
    assert_eq!(state_only.items.len(), 1);
    assert_eq!(state_only.items[0].id, "jr_l2");

    let unsupported_sync_mode = get_publish_history_with_filters(
        &app,
        DateRange {
            from: "2026-02-26 00:00:00".to_string(),
            to: "2026-02-26 23:59:59".to_string(),
        },
        PageReq {
            page: 1,
            page_size: 20,
        },
        PublishHistoryFilters {
            state: Some("failed".to_string()),
            sync_mode: Some("page_tree".to_string()),
            retryable: None,
        },
    )
    .expect("legacy unsupported sync_mode filter");
    assert_eq!(unsupported_sync_mode.total, 0);
    assert!(unsupported_sync_mode.items.is_empty());

    clear_test_env(&db_path);
}

#[test]
fn mark_source_inactive_updates_source_publish_and_sync() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);
    seed_xhs_sync_row(&conn, "inactive", "pending");
    conn.execute(
        "INSERT INTO publish_tasks(
            id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
         ) VALUES ('pt_src_inactive', 'src_inactive', 'pending', 0, NULL, NULL, NULL, '2026-02-26 09:00:00', '2026-02-26 09:00:00')",
        [],
    )
    .expect("insert publish task");
    drop(conn);

    let app = AppCore::default();
    let marked =
        mark_source_inactive(&app, "src_inactive".to_string()).expect("mark source inactive");
    assert_eq!(marked.item_id, "src_inactive");
    assert_eq!(marked.active_state, "inactive");

    let conn = open_test_conn(&db_path);
    let source_state: String = conn
        .query_row(
            "SELECT active_state FROM source_state WHERE item_id='src_inactive'",
            [],
            |row| row.get(0),
        )
        .expect("query source_state");
    assert_eq!(source_state, "inactive");

    let (publish_state, publish_error): (String, Option<String>) = conn
        .query_row(
            "SELECT state, error_code FROM publish_tasks WHERE item_id='src_inactive'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query publish task");
    assert_eq!(publish_state, "ignored");
    assert_eq!(publish_error.as_deref(), Some("source_inactive"));

    let (sync_state, sync_error): (String, Option<String>) = conn
        .query_row(
            "SELECT sync_state, last_error_code FROM sync_records WHERE id='sync_inactive'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query sync record");
    assert_eq!(sync_state, "failed");
    assert_eq!(sync_error.as_deref(), Some("source_inactive"));

    clear_test_env(&db_path);
}

#[test]
fn publish_approved_to_notion_returns_zero_when_no_candidates() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);
    drop(conn);

    let app = AppCore::default();
    let result =
        block_on(publish_approved_to_notion(&app, Some(10))).expect("publish without candidates");

    assert_eq!(result.attempted, 0);
    assert_eq!(result.succeeded, 0);
    assert_eq!(result.failed, 0);

    let conn = open_test_conn(&db_path);
    let batch_audit: (String, String) = conn
        .query_row(
            "SELECT item_id, status FROM publish_audit ORDER BY created_at DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query batch audit");
    assert_eq!(batch_audit.0, "__batch__");
    assert_eq!(batch_audit.1, "skipped");

    clear_test_env(&db_path);
}

#[test]
fn publish_approved_to_notion_marks_candidates_failed_when_sync_bootstrap_fails() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);
    seed_xhs_sync_row(&conn, "bootfail", "pending");
    drop(conn);

    let app = AppCore::default();
    let err = block_on(publish_approved_to_notion(&app, Some(1))).expect_err("publish should fail");
    assert!(err.to_string().contains("NOTION_TOKEN is required"));

    let conn = open_test_conn(&db_path);
    let (task_state, task_error_code, task_attempt_count): (String, Option<String>, i64) = conn
        .query_row(
            "SELECT state, error_code, attempt_count FROM publish_tasks WHERE item_id='src_bootfail'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("query publish task after failed publish");
    assert_eq!(task_state, "failed");
    assert_eq!(task_error_code.as_deref(), Some("IPC-6001"));
    assert_eq!(task_attempt_count, 1);

    let (sync_state, sync_error): (String, Option<String>) = conn
        .query_row(
            "SELECT sync_state, last_error_code FROM sync_records WHERE id='sync_bootfail'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query sync record after failed publish");
    assert_eq!(sync_state, "failed");
    assert_eq!(sync_error.as_deref(), Some("IPC-6001"));

    let (audit_status, audit_error_code, audit_stage, audit_mode, audit_retryable): (
        String,
        Option<String>,
        String,
        String,
        i64,
    ) = conn
        .query_row(
            "SELECT status, error_code, pipeline_stage, sync_mode, retryable
             FROM publish_audit
             WHERE item_id='src_bootfail'
             ORDER BY created_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("query audit after failed publish");
    assert_eq!(audit_status, "failed");
    assert_eq!(audit_error_code.as_deref(), Some("IPC-6001"));
    assert_eq!(audit_stage, "publish_approved_to_notion");
    assert_eq!(audit_mode, "page_tree");
    assert_eq!(audit_retryable, 0);

    clear_test_env(&db_path);
}

#[test]
fn publish_approved_to_notion_page_tree_missing_root_marks_failed_and_audits() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);
    std::env::set_var("NOTION_TOKEN", "unit-test-token");
    std::env::set_var("NOTION_SYNC_MODE", "page_tree");
    std::env::remove_var("NOTION_ROOT_PAGE_ID");

    let conn = open_test_conn(&db_path);
    seed_xhs_sync_row(&conn, "pgroot", "pending");
    drop(conn);

    let app = AppCore::default();
    let err = block_on(publish_approved_to_notion(&app, Some(1))).expect_err("publish should fail");
    assert!(err.to_string().contains("NOTION_ROOT_PAGE_ID is required"));

    let conn = open_test_conn(&db_path);
    let (task_state, task_error_code, task_attempt_count): (String, Option<String>, i64) = conn
        .query_row(
            "SELECT state, error_code, attempt_count FROM publish_tasks WHERE item_id='src_pgroot'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("query publish task after page_tree config failure");
    assert_eq!(task_state, "failed");
    assert_eq!(task_error_code.as_deref(), Some("IPC-6001"));
    assert_eq!(task_attempt_count, 1);

    let (sync_state, sync_error): (String, Option<String>) = conn
        .query_row(
            "SELECT sync_state, last_error_code FROM sync_records WHERE id='sync_pgroot'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query sync record after page_tree config failure");
    assert_eq!(sync_state, "failed");
    assert_eq!(sync_error.as_deref(), Some("IPC-6001"));

    let (audit_status, audit_error_code, audit_stage, audit_mode, audit_retryable): (
        String,
        Option<String>,
        String,
        String,
        i64,
    ) = conn
        .query_row(
            "SELECT status, error_code, pipeline_stage, sync_mode, retryable
             FROM publish_audit
             WHERE item_id='src_pgroot'
             ORDER BY created_at DESC LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("query audit after page_tree config failure");
    assert_eq!(audit_status, "failed");
    assert_eq!(audit_error_code.as_deref(), Some("IPC-6001"));
    assert_eq!(audit_stage, "publish_approved_to_notion");
    assert_eq!(audit_mode, "page_tree");
    assert_eq!(audit_retryable, 0);

    clear_test_env(&db_path);
}

#[test]
fn get_ai_usage_monthly_summary_aggregates_budget_ledger() {
    let _guard = env_lock().lock().expect("lock");
    let db_path = temp_db_path();
    set_test_env(&db_path);

    let conn = open_test_conn(&db_path);
    conn.execute(
        "INSERT INTO budget_ledger(
            id, day, provider, model, purpose, tokens_in, tokens_out, cost_cny, created_at
         ) VALUES
            ('bl_1', '2026-02-25', 'qwen', 'qwen-turbo-latest', 'b2_g2_structured_summary', 1200, 320, 0.0012, '2026-02-25 12:00:00'),
            ('bl_2', '2026-02-26', 'qwen', 'qwen-turbo-latest', 'b2_g2_structured_summary', 1800, 450, 0.0018, '2026-02-26 13:00:00'),
            ('bl_3', '2026-02-26', 'qwen', 'qwen-flash', 'b2_g2_structured_summary', 1500, 500, 0.0020, '2026-02-26 14:00:00'),
            ('bl_4', '2026-01-30', 'qwen', 'qwen-turbo-latest', 'b2_g2_structured_summary', 900, 210, 0.0009, '2026-01-30 10:00:00')",
        [],
    )
    .expect("insert budget ledger rows");
    drop(conn);

    let app = AppCore::default();
    let summary =
        get_ai_usage_monthly_summary(&app, Some("2026-02".to_string())).expect("usage summary");

    assert_eq!(summary.month, "2026-02");
    assert_eq!(summary.calls, 3);
    assert_eq!(summary.tokens_in, 4500);
    assert_eq!(summary.tokens_out, 1270);
    assert!((summary.used_cny - 0.005).abs() < 0.000001);
    assert_eq!(summary.models.len(), 2);
    assert_eq!(summary.daily.len(), 2);
    assert_eq!(summary.daily[0].day, "2026-02-26");
    assert_eq!(summary.daily[0].calls, 2);

    clear_test_env(&db_path);
}
