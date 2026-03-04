use helper_backend::app_core::{AppCore, PageReq, ReviewFilter, ReviewPatch};
use helper_backend::pipeline::normalize::{normalize, NormalizeInput};
use helper_backend::storage::db::run_migrations;
use rusqlite::{params, Connection};

fn seed_review_item(
    conn: &Connection,
    source_item_id: &str,
    normalized_item_id: &str,
    analysis_result_id: &str,
    review_id: &str,
    priority: f64,
    state: &str,
    c_score: f64,
    s_score: f64,
) {
    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
         ) VALUES (?1,'wechat',?2,'https://example.com/a',?3,'title','content',NULL,?4,'new')",
        params![
            source_item_id,
            format!("ext_{source_item_id}"),
            format!("hash_{source_item_id}"),
            "2026-02-26 10:00:00"
        ],
    )
    .expect("insert source_item");
    conn.execute(
        "INSERT INTO normalized_items(
            id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint, quality_score, value_score, gate_status, created_at, updated_at
         ) VALUES (?1,?2,'https://example.com/a',?3,'clean',?4,80.0,0.6,'review',?5,?5)",
        params![
            normalized_item_id,
            source_item_id,
            format!("chash_{normalized_item_id}"),
            format!("fp_{normalized_item_id}"),
            "2026-02-26 10:00:00"
        ],
    )
    .expect("insert normalized");
    conn.execute(
        "INSERT INTO analysis_results(
            id, normalized_item_id, classification_json, summary_text, c_score, s_score, j_score, d_score, value_score, quality_score, review_required, review_status, final_status, created_at, updated_at
         ) VALUES (?1,?2,'{}','summary',?3,?4,0.8,0.7,0.6,79.0,1,'pending','review',?5,?5)",
        params![
            analysis_result_id,
            normalized_item_id,
            c_score,
            s_score,
            "2026-02-26 10:00:00"
        ],
    )
    .expect("insert analysis");
    conn.execute(
        "INSERT INTO review_queue(
            id, normalized_item_id, analysis_result_id, priority, reason, state, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,'needs review',?5,?6,?6)",
        params![
            review_id,
            normalized_item_id,
            analysis_result_id,
            priority,
            state,
            "2026-02-26 10:00:00"
        ],
    )
    .expect("insert review_queue");
}

#[test]
fn empty_db_returns_zero_dashboard_and_empty_review_page() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    let metrics = app
        .get_dashboard_metrics_with_conn(&conn)
        .expect("dashboard metrics");
    assert_eq!(metrics.today_collected, 0);
    assert_eq!(metrics.pending_review, 0);
    assert_eq!(metrics.classification_accuracy, 0.0);
    assert_eq!(metrics.summary_usability, 0.0);

    let page = app
        .get_review_items_with_conn(
            &conn,
            ReviewFilter {
                state: None,
                min_priority: None,
            },
            PageReq {
                page: 1,
                page_size: 20,
            },
        )
        .expect("review page");
    assert_eq!(page.total, 0);
    assert!(page.items.is_empty());
}

#[test]
fn normal_data_collect_and_dashboard_read_from_db() {
    let mut conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    seed_review_item(
        &conn, "src_a", "norm_a", "ana_a", "rv_a", 0.9, "pending", 0.82, 0.78,
    );
    seed_review_item(
        &conn, "src_b", "norm_b", "ana_b", "rv_b", 0.4, "done", 0.74, 0.7,
    );

    conn.execute(
        "UPDATE source_items SET collected_at='2026-02-20 10:00:00' WHERE id='src_b'",
        [],
    )
    .expect("update collected_at");

    let collect = app
        .collect_with_conn(
            &mut conn,
            "wechat",
            Some("2026-02-25 00:00:00"),
            Some("2026-02-26 23:59:59"),
        )
        .expect("collect");
    assert_eq!(collect.source, "wechat");
    assert_eq!(collect.fetched, 1);
    assert_eq!(collect.stored, 1);

    let metrics = app
        .get_dashboard_metrics_with_conn(&conn)
        .expect("dashboard metrics");
    assert_eq!(metrics.pending_review, 1);
    assert!(metrics.classification_accuracy > 0.0);
    assert!(metrics.summary_usability > 0.0);
}

#[test]
fn review_queue_pagination_and_update_are_db_driven() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    seed_review_item(
        &conn, "src_p1", "norm_p1", "ana_p1", "rv_p1", 0.9, "pending", 0.83, 0.79,
    );
    seed_review_item(
        &conn, "src_p2", "norm_p2", "ana_p2", "rv_p2", 0.7, "pending", 0.81, 0.75,
    );
    seed_review_item(
        &conn, "src_p3", "norm_p3", "ana_p3", "rv_p3", 0.2, "done", 0.7, 0.68,
    );

    let first_page = app
        .get_review_items_with_conn(
            &conn,
            ReviewFilter {
                state: Some("pending".to_string()),
                min_priority: None,
            },
            PageReq {
                page: 1,
                page_size: 1,
            },
        )
        .expect("first page");
    assert_eq!(first_page.total, 2);
    assert_eq!(first_page.items.len(), 1);
    assert_eq!(first_page.items[0].id, "rv_p1");
    assert_eq!(first_page.items[0].title.as_deref(), Some("title"));
    assert_eq!(
        first_page.items[0].url.as_deref(),
        Some("https://example.com/a")
    );

    let second_page = app
        .get_review_items_with_conn(
            &conn,
            ReviewFilter {
                state: Some("pending".to_string()),
                min_priority: None,
            },
            PageReq {
                page: 2,
                page_size: 1,
            },
        )
        .expect("second page");
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].id, "rv_p2");

    let updated = app
        .update_review_item_with_conn(
            &conn,
            "rv_p2",
            ReviewPatch {
                state: Some("done".to_string()),
                note: Some("manual-pass".to_string()),
            },
        )
        .expect("update review item");
    assert_eq!(updated.id, "rv_p2");
    assert_eq!(updated.state, "done");
    assert_eq!(updated.reason, "manual-pass");
    assert_eq!(updated.title.as_deref(), Some("title"));
}

#[test]
fn retry_failed_items_requeues_non_pending_and_tracks_ignored() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    seed_review_item(
        &conn, "src_r1", "norm_r1", "ana_r1", "rv_r1", 0.5, "done", 0.7, 0.7,
    );
    seed_review_item(
        &conn, "src_r2", "norm_r2", "ana_r2", "rv_r2", 0.6, "pending", 0.7, 0.7,
    );

    let result = app
        .retry_failed_items_with_conn(
            &conn,
            &[
                "rv_r1".to_string(),
                "rv_r2".to_string(),
                "missing".to_string(),
            ],
        )
        .expect("retry failed items");
    assert_eq!(result.requested, 3);
    assert_eq!(result.requeued, 1);
    assert_eq!(result.ignored, 2);

    let state: String = conn
        .query_row("SELECT state FROM review_queue WHERE id='rv_r1'", [], |r| {
            r.get(0)
        })
        .expect("query state");
    assert_eq!(state, "pending");
}

#[test]
fn xhs_policy_auto_pass_moves_legacy_pending_to_done() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();
    let now = "2026-02-26 10:00:00";

    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
         ) VALUES ('src_xhs_legacy','xhs','ext_xhs_legacy','https://www.xiaohongshu.com/explore/a','hash_xhs_legacy','legacy','这是一条足够长的小红书内容用于自动通过策略验证',NULL,?1,'new')",
        params![now],
    )
    .expect("insert source");
    conn.execute(
        "INSERT INTO normalized_items(
            id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint, quality_score, value_score, gate_status, created_at, updated_at
         ) VALUES ('norm_xhs_legacy','src_xhs_legacy','https://www.xiaohongshu.com/explore/a','chash_xhs_legacy','这是一条足够长的小红书内容用于自动通过策略验证','fp_xhs_legacy',79.0,0.75,'review',?1,?1)",
        params![now],
    )
    .expect("insert normalized");
    conn.execute(
        "INSERT INTO analysis_results(
            id, normalized_item_id, classification_json, summary_text, c_score, s_score, j_score, d_score, value_score, quality_score, review_required, review_status, final_status, created_at, updated_at
         ) VALUES ('ana_xhs_legacy','norm_xhs_legacy','{}','summary',0.82,0.79,0.8,0.75,0.75,79.0,1,'pending','review',?1,?1)",
        params![now],
    )
    .expect("insert analysis");
    conn.execute(
        "INSERT INTO review_queue(
            id, normalized_item_id, analysis_result_id, priority, reason, state, created_at, updated_at
         ) VALUES ('rvw_xhs_legacy','norm_xhs_legacy','ana_xhs_legacy',0.60,'XHS source requires manual review','pending',?1,?1)",
        params![now],
    )
    .expect("insert review queue");

    let pending_page = app
        .get_review_items_with_conn(
            &conn,
            ReviewFilter {
                state: Some("pending".to_string()),
                min_priority: None,
            },
            PageReq {
                page: 1,
                page_size: 20,
            },
        )
        .expect("pending page");
    assert_eq!(pending_page.total, 0);

    let done_page = app
        .get_review_items_with_conn(
            &conn,
            ReviewFilter {
                state: Some("done".to_string()),
                min_priority: None,
            },
            PageReq {
                page: 1,
                page_size: 20,
            },
        )
        .expect("done page");
    assert_eq!(done_page.total, 1);
    assert_eq!(done_page.items[0].reason, "XHS auto-pass normal item");
}

#[test]
fn xhs_collect_skips_existing_fingerprint_and_continues() {
    let _guard = XHS_ENV_LOCK.lock().expect("lock");
    let mut conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    let duplicated = normalize(&NormalizeInput {
        source: "xhs".to_string(),
        source_url: "https://www.xiaohongshu.com/explore/dup".to_string(),
        published_at: Some("2026-02-26 10:00:00".to_string()),
        collected_at: "2026-02-26 10:30:00".to_string(),
        content_raw: "重复内容".to_string(),
    });
    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
         ) VALUES ('src_dup','xhs','ext_dup',?1,?2,'dup','重复内容','2026-02-26 10:00:00','2026-02-26 10:30:00','new')",
        params![duplicated.canonical_url, duplicated.url_hash],
    )
    .expect("seed source");
    conn.execute(
        "INSERT INTO normalized_items(
            id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint, quality_score, value_score, gate_status, created_at, updated_at
         ) VALUES ('norm_dup','src_dup',?1,?2,'重复内容',?3,70,0.5,'review','2026-02-26 10:30:00','2026-02-26 10:30:00')",
        params![duplicated.canonical_url, duplicated.url_hash, duplicated.fingerprint],
    )
    .expect("seed normalized");

    std::env::set_var(
        "HELPER_XHS_FAKE_ITEMS_JSON",
        r#"[
          {"external_id":"xhs_new","source_url":"https://www.xiaohongshu.com/explore/new","title":"新内容","content_raw":"新内容主体","published_at":"2026-02-26 12:00:00"},
          {"external_id":"xhs_dup","source_url":"https://www.xiaohongshu.com/explore/dup","title":"重复内容","content_raw":"重复内容","published_at":"2026-02-26 11:00:00"},
          {"external_id":"xhs_after","source_url":"https://www.xiaohongshu.com/explore/after","title":"不应入库","content_raw":"不应入库","published_at":"2026-02-26 09:00:00"}
        ]"#,
    );

    let result = app
        .collect_with_conn(&mut conn, "xhs", None, None)
        .expect("collect xhs");
    std::env::remove_var("HELPER_XHS_FAKE_ITEMS_JSON");

    assert_eq!(result.fetched, 3);
    assert_eq!(result.stored, 2);

    let xhs_count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM source_items WHERE source='xhs'",
            [],
            |row| row.get(0),
        )
        .expect("query xhs count");
    assert_eq!(xhs_count, 3);

    let done_count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM review_queue WHERE state='done'",
            [],
            |row| row.get(0),
        )
        .expect("query done count");
    assert_eq!(done_count, 2);
}

static XHS_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
