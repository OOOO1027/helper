use helper_backend::app_core::AppCore;
use helper_backend::storage::db::run_migrations;
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file(name: &str, content: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let mut p = std::env::temp_dir();
    let name = if let Some((stem, ext)) = name.rsplit_once('.') {
        format!("helper_backend_dlq_{stem}_{ts}.{ext}")
    } else {
        format!("helper_backend_dlq_{name}_{ts}")
    };
    p.push(name);
    fs::write(&p, content).expect("write temp file");
    p.to_string_lossy().to_string()
}

fn remove_if_exists(path: &str) {
    let p = PathBuf::from(path);
    if p.exists() {
        let _ = fs::remove_file(p);
    }
}

#[test]
fn retry_dead_letter_requeues_sync_record_and_resolves_dead_letter() {
    let mut conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let now = "2026-02-25 21:30:00";

    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
         ) VALUES ('src_a','wechat','ext_a','https://example.com/a','ha','t','body',?1,?1,'new')",
        params![now],
    )
    .expect("insert source");
    conn.execute(
        "INSERT INTO normalized_items(
            id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint, quality_score, value_score, gate_status, created_at, updated_at
         ) VALUES ('norm_a','src_a','https://example.com/a','ha','body','fp',80,0.8,'direct',?1,?1)",
        params![now],
    )
    .expect("insert normalized");
    conn.execute(
        "INSERT INTO sync_records(
            id, normalized_item_id, target, sync_state, retry_count, next_retry_at, created_at, updated_at
         ) VALUES ('sync_a','norm_a','notion','failed',3,NULL,?1,?1)",
        params![now],
    )
    .expect("insert sync");
    conn.execute(
        "INSERT INTO dead_letter(
            id, entity_type, entity_id, stage, payload_json, error_code, error_message, attempt_count, state, created_at
         ) VALUES ('dlq_a','sync_record','sync_a','notion_sync','{}','MOD-3002','failed',3,'open',?1)",
        params![now],
    )
    .expect("insert dlq");
    conn.execute(
        "INSERT INTO publish_tasks(
            id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
         ) VALUES ('pt_src_a', 'src_a', 'failed', 3, NULL, 'MOD-3002', 'failed', ?1, ?1)",
        params![now],
    )
    .expect("insert publish task");

    let app = AppCore::default();
    let result = app
        .retry_dead_letters_with_conn(&mut conn, &["dlq_a".to_string()])
        .expect("retry dlq");
    assert_eq!(result.requested, 1);
    assert_eq!(result.requeued, 1);
    assert_eq!(result.ignored, 0);

    let sync_state: String = conn
        .query_row(
            "SELECT sync_state FROM sync_records WHERE id='sync_a'",
            [],
            |r| r.get(0),
        )
        .expect("query sync state");
    assert_eq!(sync_state, "retry");
    let (publish_state, publish_error): (String, Option<String>) = conn
        .query_row(
            "SELECT state, error_code FROM publish_tasks WHERE id='pt_src_a'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query publish task");
    assert_eq!(publish_state, "pending");
    assert_eq!(publish_error, None);

    let dlq_state: String = conn
        .query_row("SELECT state FROM dead_letter WHERE id='dlq_a'", [], |r| {
            r.get(0)
        })
        .expect("query dlq state");
    assert_eq!(dlq_state, "resolved");
    let replay_count: i64 = conn
        .query_row(
            "SELECT replay_count FROM dead_letter WHERE id='dlq_a'",
            [],
            |r| r.get(0),
        )
        .expect("query dlq replay_count");
    assert_eq!(replay_count, 1);
    let replay_by: String = conn
        .query_row(
            "SELECT last_replayed_by FROM dead_letter WHERE id='dlq_a'",
            [],
            |r| r.get(0),
        )
        .expect("query replay by");
    assert_eq!(replay_by, "system");
}

#[test]
fn retry_dead_letter_replays_wechat_import_parse_path() {
    let mut conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let now = "2026-02-25 21:30:00";
    let path = temp_file(
        "replay.txt",
        "发布时间 2026-02-25 21:30:00 https://example.com/r?id=1 replay",
    );

    conn.execute(
        "INSERT INTO dead_letter(
            id, entity_type, entity_id, stage, payload_json, error_code, error_message, attempt_count, state, created_at
         ) VALUES ('dlq_replay','pipeline',?1,'wechat_import_parse','{}','PAR-2001','parse failed',1,'open',?2)",
        params![path, now],
    )
    .expect("insert dlq replay");

    let app = AppCore::default();
    let result = app
        .retry_dead_letters_with_conn(&mut conn, &["dlq_replay".to_string()])
        .expect("retry dlq replay");
    assert_eq!(result.requested, 1);
    assert_eq!(result.requeued, 1);
    assert_eq!(result.ignored, 0);

    let dlq_state: String = conn
        .query_row(
            "SELECT state FROM dead_letter WHERE id='dlq_replay'",
            [],
            |r| r.get(0),
        )
        .expect("query dlq replay state");
    assert_eq!(dlq_state, "resolved");
    let replay_status: String = conn
        .query_row(
            "SELECT last_replay_status FROM dead_letter WHERE id='dlq_replay'",
            [],
            |r| r.get(0),
        )
        .expect("query replay status");
    assert_eq!(replay_status, "resolved");
    let latency_ms: i64 = conn
        .query_row(
            "SELECT last_replay_latency_ms FROM dead_letter WHERE id='dlq_replay'",
            [],
            |r| r.get(0),
        )
        .expect("query replay latency");
    assert!(latency_ms >= 0);

    let source_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM source_items", [], |r| r.get(0))
        .expect("count source_items");
    assert_eq!(source_count, 1);

    remove_if_exists(&path);
}

#[test]
fn retry_dead_letter_replays_pipeline_execute_from_payload() {
    let mut conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let now = "2026-02-25 21:30:00";
    let payload = r#"{"source":"wechat","source_url":"https://example.com/p?id=8","published_at":"2026-02-25T21:30:00+08:00","content_raw":"payload replay"}"#;

    conn.execute(
        "INSERT INTO dead_letter(
            id, entity_type, entity_id, stage, payload_json, error_code, error_message, attempt_count, state, created_at
         ) VALUES ('dlq_payload','pipeline','unknown','pipeline_execute',?1,'DB-4001','failed',1,'open',?2)",
        params![payload, now],
    )
    .expect("insert dlq payload");

    let app = AppCore::default();
    let result = app
        .retry_dead_letters_with_conn(&mut conn, &["dlq_payload".to_string()])
        .expect("retry payload dlq");
    assert_eq!(result.requeued, 1);
    assert_eq!(result.ignored, 0);

    let source_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM source_items", [], |r| r.get(0))
        .expect("count source after payload replay");
    assert_eq!(source_count, 1);
}
