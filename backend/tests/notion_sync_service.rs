use async_trait::async_trait;
use futures::executor::block_on;
use helper_backend::storage::db::run_migrations;
use helper_backend::sync_notion::client::{NotionClient, NotionRecord, SyncResult};
use helper_backend::sync_notion::service::{
    sync_pending_with_conn, sync_pending_with_conn_with_options,
};
use helper_backend::{BackendError, Result};
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::sync::Mutex;

struct MockNotionClient {
    fail_ids: Mutex<HashSet<String>>,
    error_message: String,
}

impl MockNotionClient {
    fn new(fail_ids: HashSet<String>) -> Self {
        Self {
            fail_ids: Mutex::new(fail_ids),
            error_message: "mock notion failure".to_string(),
        }
    }

    fn new_with_error(fail_ids: HashSet<String>, error_message: &str) -> Self {
        Self {
            fail_ids: Mutex::new(fail_ids),
            error_message: error_message.to_string(),
        }
    }
}

#[async_trait]
impl NotionClient for MockNotionClient {
    async fn upsert_batch(&self, records: &[NotionRecord]) -> Result<SyncResult> {
        let fail_ids = self.fail_ids.lock().expect("lock fail ids");
        let should_fail = records
            .iter()
            .any(|r| fail_ids.contains(&r.normalized_item_id));
        if should_fail {
            return Err(BackendError::Internal(self.error_message.clone()));
        }
        Ok(SyncResult {
            success: records.len(),
            failed: 0,
        })
    }
}

fn seed_sync_row(conn: &Connection, suffix: &str, retry_count: i64) {
    let source_id = format!("src_{suffix}");
    let normalized_id = format!("norm_{suffix}");
    let analysis_id = format!("ana_{suffix}");
    let sync_id = format!("sync_{suffix}");
    let now = "2026-02-25 21:30:00";
    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
         ) VALUES (?1,'wechat',?2,?3,?4,?5,?6,?7,?8,'new')",
        params![
            source_id,
            format!("ext_{suffix}"),
            "https://example.com/a",
            format!("h_{suffix}"),
            "t",
            "body",
            now,
            now
        ],
    )
    .expect("insert source_items");
    conn.execute(
        "INSERT INTO normalized_items(
            id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint, quality_score, value_score, gate_status, created_at, updated_at
         ) VALUES (?1,?2,'https://example.com/a',?3,'body','fp',80,0.8,'direct',?4,?4)",
        params![normalized_id, source_id, format!("uh_{suffix}"), now],
    )
    .expect("insert normalized_items");
    conn.execute(
        "INSERT INTO analysis_results(
            id, normalized_item_id, classification_json, summary_text, c_score, s_score, j_score, d_score, value_score, quality_score, review_required, review_status, final_status, created_at, updated_at
         ) VALUES (?1,?2,'[\"后端工程\"]','summary',0.9,0.9,0.8,0.8,0.85,82,0,'skipped','direct',?3,?3)",
        params![analysis_id, normalized_id, now],
    )
    .expect("insert analysis_results");
    conn.execute(
        "INSERT INTO sync_records(
            id, normalized_item_id, target, sync_state, retry_count, next_retry_at, created_at, updated_at
         ) VALUES (?1,?2,'notion','pending',?3,NULL,?4,?4)",
        params![sync_id, normalized_id, retry_count, now],
    )
    .expect("insert sync_records");
}

#[test]
fn sync_pending_marks_success() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "ok", 0);

    let client = MockNotionClient::new(HashSet::new());
    let summary = block_on(sync_pending_with_conn(&conn, &client, 10)).expect("sync run");
    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.succeeded, 1);
    assert_eq!(summary.failed, 0);

    let state: String = conn
        .query_row(
            "SELECT sync_state FROM sync_records WHERE id='sync_ok'",
            [],
            |r| r.get(0),
        )
        .expect("query sync state");
    assert_eq!(state, "success");
}

#[test]
fn sync_pending_moves_to_dead_letter_after_retry_exhausted() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "fail", 2);

    let mut fail = HashSet::new();
    fail.insert("norm_fail".to_string());
    let client = MockNotionClient::new(fail);
    let summary = block_on(sync_pending_with_conn(&conn, &client, 10)).expect("sync run");
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.dead_lettered, 1);

    let state: String = conn
        .query_row(
            "SELECT sync_state FROM sync_records WHERE id='sync_fail'",
            [],
            |r| r.get(0),
        )
        .expect("query sync state");
    assert_eq!(state, "failed");

    let dlq_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM dead_letter", [], |r| r.get(0))
        .expect("query dead_letter count");
    assert!(dlq_count >= 1);
}

#[test]
fn sync_pending_rate_limited_failure_maps_mod_3429_and_requeues() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "retrying", 0);

    let mut fail = HashSet::new();
    fail.insert("norm_retrying".to_string());
    let client =
        MockNotionClient::new_with_error(fail, "notion upsert non-success 429: rate_limited");
    let summary =
        block_on(sync_pending_with_conn_with_options(&conn, &client, 10, 0)).expect("sync run");
    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.requeued, 1);
    assert_eq!(summary.dead_lettered, 0);

    let (sync_state, retry_count, next_retry_at, error_code): (
        String,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT sync_state, retry_count, next_retry_at, last_error_code
             FROM sync_records
             WHERE id='sync_retrying'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query sync retry state");
    assert_eq!(sync_state, "retry");
    assert_eq!(retry_count, 1);
    assert!(next_retry_at.is_some());
    assert_eq!(error_code.as_deref(), Some("MOD-3429"));

    let (task_state, attempt_count, task_next_retry_at, task_error_code): (
        String,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT state, attempt_count, next_retry_at, error_code
             FROM publish_tasks
             WHERE id='pt_src_retrying'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query publish task retry state");
    assert_eq!(task_state, "failed");
    assert_eq!(attempt_count, 1);
    assert!(task_next_retry_at.is_some());
    assert_eq!(task_error_code.as_deref(), Some("MOD-3429"));
}

#[test]
fn sync_pending_respects_limit_option() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "l1", 0);
    seed_sync_row(&conn, "l2", 0);

    let client = MockNotionClient::new(HashSet::new());
    let summary = block_on(sync_pending_with_conn_with_options(&conn, &client, 1, 0))
        .expect("sync run with limit");
    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.succeeded, 1);

    let pending_count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM sync_records WHERE sync_state='pending'",
            [],
            |r| r.get(0),
        )
        .expect("count pending");
    assert_eq!(pending_count, 1);
}

#[test]
fn sync_pending_skips_future_retry_items() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "future", 1);

    conn.execute(
        "UPDATE sync_records
         SET sync_state='retry',
             retry_count=1,
             next_retry_at='2099-01-01 00:00:00',
             updated_at='2026-02-25 21:40:00'
         WHERE id='sync_future'",
        [],
    )
    .expect("set future retry");

    let client = MockNotionClient::new(HashSet::new());
    let summary =
        block_on(sync_pending_with_conn_with_options(&conn, &client, 10, 0)).expect("sync run");
    assert_eq!(summary.scanned, 0);
    assert_eq!(summary.attempted, 0);
    assert_eq!(summary.succeeded, 0);
    assert_eq!(summary.failed, 0);

    let (task_state, next_retry_at): (String, Option<String>) = conn
        .query_row(
            "SELECT state, next_retry_at
             FROM publish_tasks
             WHERE id='pt_src_future'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query future publish task");
    assert_eq!(task_state, "failed");
    assert_eq!(next_retry_at.as_deref(), Some("2099-01-01 00:00:00"));
}

#[test]
fn sync_pending_allows_requeue_after_published_when_sync_record_reset() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "requeue", 0);

    let client = MockNotionClient::new(HashSet::new());
    let first =
        block_on(sync_pending_with_conn_with_options(&conn, &client, 10, 0)).expect("first sync");
    assert_eq!(first.succeeded, 1);

    conn.execute(
        "UPDATE sync_records
         SET sync_state='pending',
             retry_count=0,
             next_retry_at=NULL,
             updated_at='2026-02-25 21:40:00'
         WHERE id='sync_requeue'",
        [],
    )
    .expect("requeue sync_records");

    let second =
        block_on(sync_pending_with_conn_with_options(&conn, &client, 10, 0)).expect("second sync");
    assert_eq!(second.scanned, 1);
    assert_eq!(second.succeeded, 1);
}

#[test]
fn sync_pending_keeps_await_review_ignored() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "await", 0);

    conn.execute(
        "INSERT INTO publish_tasks(
            id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
         ) VALUES ('pt_src_await', 'src_await', 'ignored', 0, NULL, 'await_review', 'await manual review', '2026-02-25 21:30:00', '2026-02-25 21:30:00')",
        [],
    )
    .expect("insert await_review task");

    let client = MockNotionClient::new(HashSet::new());
    let summary =
        block_on(sync_pending_with_conn_with_options(&conn, &client, 10, 0)).expect("sync run");
    assert_eq!(summary.scanned, 0);
    assert_eq!(summary.succeeded, 0);
}

#[test]
fn sync_pending_recovers_stuck_processing_task() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(&conn, "stuck", 0);

    conn.execute(
        "INSERT INTO publish_tasks(
            id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
         ) VALUES ('pt_src_stuck', 'src_stuck', 'processing', 1, NULL, NULL, NULL, '2026-02-25 20:00:00', '2026-02-25 20:00:00')",
        [],
    )
    .expect("insert stuck processing task");

    let client = MockNotionClient::new(HashSet::new());
    let summary =
        block_on(sync_pending_with_conn_with_options(&conn, &client, 10, 0)).expect("sync run");
    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.succeeded, 1);

    let state: String = conn
        .query_row(
            "SELECT state FROM publish_tasks WHERE id='pt_src_stuck'",
            [],
            |r| r.get(0),
        )
        .expect("query publish task state");
    assert_eq!(state, "published");
}
