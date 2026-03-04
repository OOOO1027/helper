use helper_backend::app_core::AppCore;
use helper_backend::storage::db::run_migrations;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file(name: &str, content: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let mut p = std::env::temp_dir();
    let file_name = match name.rsplit_once('.') {
        Some((stem, ext)) => format!("helper_backend_flow_{stem}_{ts}.{ext}"),
        None => format!("helper_backend_flow_{name}_{ts}"),
    };
    p.push(file_name);
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
fn import_wechat_and_persist_end_to_end_summary() {
    let mut conn = Connection::open_in_memory().expect("open memory db");
    run_migrations(&conn).expect("migrations");

    let p1 = temp_file(
        "w1.txt",
        "发布时间 2026-02-25 21:30:00 https://example.com/post?id=1 hello",
    );
    let p2 = temp_file(
        "w2.txt",
        "发布时间 2026-02-25 21:30:00 https://example.com/post?id=1 hello",
    );
    let p3 = temp_file("w3.html", "<p>另一个内容 https://example.com/post?id=2</p>");
    let p4 = temp_file("w4.pdf", "unsupported");

    let app = AppCore::default();
    let result = app
        .import_wechat_and_persist_with_conn(
            &mut conn,
            &[p1.clone(), p2.clone(), p3.clone(), p4.clone()],
        )
        .expect("import and persist");

    assert_eq!(result.total, 4);
    assert_eq!(result.parsed_ok, 2);
    assert_eq!(result.duplicates, 1);
    assert_eq!(result.parse_failed, 1);
    assert_eq!(result.persisted, 2);
    assert_eq!(result.persist_failed, 0);

    let source_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM source_items", [], |r| r.get(0))
        .expect("count source_items");
    let sync_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM sync_records", [], |r| r.get(0))
        .expect("count sync_records");
    let dead_letter_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM dead_letter", [], |r| r.get(0))
        .expect("count dead_letter");
    assert_eq!(source_count, 2);
    assert_eq!(sync_count, 2);
    assert!(dead_letter_count >= 1);

    remove_if_exists(&p1);
    remove_if_exists(&p2);
    remove_if_exists(&p3);
    remove_if_exists(&p4);
}
