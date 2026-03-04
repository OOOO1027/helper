use helper_backend::app_core::{AppCore, PipelinePreviewReq};
use helper_backend::storage::db::run_migrations;
use rusqlite::Connection;

#[test]
fn execute_pipeline_is_idempotent_for_same_input() {
    let mut conn = Connection::open_in_memory().expect("open memory db");
    run_migrations(&conn).expect("migrations");

    let app = AppCore::default();
    let req = PipelinePreviewReq {
        source: "wechat".to_string(),
        source_url: Some("https://example.com/post?id=1".to_string()),
        published_at: Some("2026-02-25T21:30:00+08:00".to_string()),
        content_raw: "Rust Tauri 后端写入 Notion".to_string(),
    };

    let first = app
        .execute_pipeline_with_conn(&mut conn, req.clone())
        .expect("first execute");
    assert_eq!(first.notion_action, "create");

    let second = app
        .execute_pipeline_with_conn(&mut conn, req)
        .expect("second execute");
    assert_eq!(second.notion_action, "update");
    assert_eq!(first.normalized_item_id, second.normalized_item_id);

    let source_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM source_items", [], |r| r.get(0))
        .expect("count source_items");
    let normalized_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM normalized_items", [], |r| r.get(0))
        .expect("count normalized_items");
    let analysis_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM analysis_results", [], |r| r.get(0))
        .expect("count analysis_results");
    let sync_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM sync_records", [], |r| r.get(0))
        .expect("count sync_records");
    assert_eq!(source_count, 1);
    assert_eq!(normalized_count, 1);
    assert_eq!(analysis_count, 1);
    assert_eq!(sync_count, 1);
}
