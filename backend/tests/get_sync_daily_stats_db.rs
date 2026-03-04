use helper_backend::app_core::{AppCore, DateRange, PageReq};
use helper_backend::storage::db::run_migrations;
use rusqlite::{params, Connection};

#[test]
fn get_sync_daily_stats_with_conn_reads_view_rows() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");

    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr1','notion_sync_once','notion','success',?1,?1,3,0,'{}',?1)",
        params!["2026-02-26 09:00:00"],
    )
    .expect("insert jr1");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr2','notion_smoke','notion','partial',?1,?1,1,1,'{}',?1)",
        params!["2026-02-26 09:10:00"],
    )
    .expect("insert jr2");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr3','notion_sync_once','notion','failed',?1,?1,0,1,'{}',?1)",
        params!["2026-02-25 20:00:00"],
    )
    .expect("insert jr3");

    let app = AppCore::default();
    let page = app
        .get_sync_daily_stats_with_conn(
            &conn,
            DateRange {
                from: "2026-02-25 00:00:00".to_string(),
                to: "2026-02-26 23:59:59".to_string(),
            },
            PageReq {
                page: 1,
                page_size: 10,
            },
            None,
            None,
        )
        .expect("get stats");
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.items[0].day, "2026-02-26");
    assert!(page
        .items
        .iter()
        .any(|v| v.job_type == "notion_smoke" && v.partial_runs == 1));
}

#[test]
fn get_sync_daily_stats_with_conn_supports_job_type_and_status_filters() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");

    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr10','notion_sync_once','notion','success',?1,?1,2,0,'{}',?1)",
        params!["2026-02-26 08:00:00"],
    )
    .expect("insert jr10");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr11','notion_sync_once','notion','failed',?1,?1,0,1,'{}',?1)",
        params!["2026-02-26 09:00:00"],
    )
    .expect("insert jr11");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr12','notion_smoke','notion','partial',?1,?1,1,1,'{}',?1)",
        params!["2026-02-26 09:30:00"],
    )
    .expect("insert jr12");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr13','notion_smoke','notion','skipped',?1,?1,0,0,'{}',?1)",
        params!["2026-02-25 20:00:00"],
    )
    .expect("insert jr13");

    let app = AppCore::default();
    let range = DateRange {
        from: "2026-02-25 00:00:00".to_string(),
        to: "2026-02-26 23:59:59".to_string(),
    };
    let page = PageReq {
        page: 1,
        page_size: 10,
    };

    let by_job_type = app
        .get_sync_daily_stats_with_conn(
            &conn,
            range.clone(),
            page.clone(),
            Some(" notion_smoke ".to_string()),
            None,
        )
        .expect("query by job_type");
    assert_eq!(by_job_type.total, 2);
    assert!(by_job_type
        .items
        .iter()
        .all(|v| v.job_type == "notion_smoke"));

    let by_failed_status = app
        .get_sync_daily_stats_with_conn(
            &conn,
            range.clone(),
            page.clone(),
            None,
            Some("failed".to_string()),
        )
        .expect("query by failed status");
    assert_eq!(by_failed_status.total, 1);
    assert_eq!(by_failed_status.items[0].job_type, "notion_sync_once");
    assert_eq!(by_failed_status.items[0].failed_runs, 1);

    let by_skipped_status = app
        .get_sync_daily_stats_with_conn(&conn, range, page, None, Some("skipped".to_string()))
        .expect("query by skipped status");
    assert_eq!(by_skipped_status.total, 1);
    assert_eq!(by_skipped_status.items[0].job_type, "notion_smoke");
    assert_eq!(by_skipped_status.items[0].day, "2026-02-25");
}

#[test]
fn get_sync_daily_stats_with_conn_rejects_invalid_status_filter() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    let err = app
        .get_sync_daily_stats_with_conn(
            &conn,
            DateRange {
                from: "2026-02-25 00:00:00".to_string(),
                to: "2026-02-26 23:59:59".to_string(),
            },
            PageReq {
                page: 1,
                page_size: 10,
            },
            None,
            Some("pending".to_string()),
        )
        .expect_err("invalid status should fail");
    assert!(err.to_string().contains("status must be one of"));
}

#[test]
fn get_sync_daily_stats_with_conn_rejects_empty_range() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    let err = app
        .get_sync_daily_stats_with_conn(
            &conn,
            DateRange {
                from: "".to_string(),
                to: "".to_string(),
            },
            PageReq {
                page: 1,
                page_size: 10,
            },
            None,
            None,
        )
        .expect_err("empty range should fail");
    assert!(err
        .to_string()
        .contains("range.from and range.to must not be empty"));
}

#[test]
fn get_sync_daily_stats_with_conn_rejects_invalid_page() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    let app = AppCore::default();

    let err = app
        .get_sync_daily_stats_with_conn(
            &conn,
            DateRange {
                from: "2026-02-25 00:00:00".to_string(),
                to: "2026-02-26 23:59:59".to_string(),
            },
            PageReq {
                page: 0,
                page_size: 10,
            },
            None,
            None,
        )
        .expect_err("invalid page should fail");
    assert!(err
        .to_string()
        .contains("page and page_size must be positive"));
}
