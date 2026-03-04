use helper_backend::app_core::{AppCore, DateRange, PageReq};
use helper_backend::storage::db::run_migrations;
use rusqlite::{params, Connection};

#[test]
fn get_sync_logs_with_conn_reads_sync_records_and_smoke_job_runs() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let now = "2026-02-26 00:01:00";

    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
         ) VALUES ('src_a','wechat','ext_a','https://example.com/a','ha','title','body',?1,?1,'new')",
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
         ) VALUES ('sync_a','norm_a','notion','failed',1,NULL,?1,?1)",
        params![now],
    )
    .expect("insert sync");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr_a','notion_smoke','notion','partial',?1,?1,1,1,'{}',?1)",
        params![now],
    )
    .expect("insert job_run");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr_b','notion_sync_once','notion','success',?1,?1,2,0,'{}',?1)",
        params![now],
    )
    .expect("insert notion_sync_once job_run");

    let app = AppCore::default();
    let page = app
        .get_sync_logs_with_conn(
            &conn,
            DateRange {
                from: "2026-02-26 00:00:00".to_string(),
                to: "2026-02-26 23:59:59".to_string(),
            },
            PageReq {
                page: 1,
                page_size: 10,
            },
        )
        .expect("get sync logs");

    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
    let states: Vec<String> = page.items.into_iter().map(|v| v.state).collect();
    assert!(states.contains(&"failed".to_string()));
    assert!(states.contains(&"partial".to_string()));
    assert!(states.contains(&"success".to_string()));
}
