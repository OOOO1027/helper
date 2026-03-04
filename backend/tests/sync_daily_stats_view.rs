use helper_backend::storage::db::run_migrations;
use rusqlite::{params, Connection};

#[test]
fn sync_daily_stats_view_aggregates_by_day_and_job_type() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");

    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr1','notion_sync_once','notion','success',?1,?1,3,0,'{}',?1)",
        params!["2026-02-26 00:10:00"],
    )
    .expect("insert jr1");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr2','notion_sync_once','notion','partial',?1,?1,2,1,'{}',?1)",
        params!["2026-02-26 00:12:00"],
    )
    .expect("insert jr2");
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count, metadata_json, created_at
         ) VALUES ('jr3','notion_smoke','notion','failed',?1,?1,0,1,'{}',?1)",
        params!["2026-02-26 00:15:00"],
    )
    .expect("insert jr3");

    let (run_count, success_runs, partial_runs, failed_runs): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT run_count, success_runs, partial_runs, failed_runs
             FROM v_sync_daily_stats
             WHERE day='2026-02-26' AND job_type='notion_sync_once'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query notion_sync_once stats");
    assert_eq!(run_count, 2);
    assert_eq!(success_runs, 1);
    assert_eq!(partial_runs, 1);
    assert_eq!(failed_runs, 0);

    let smoke_failed_runs: i64 = conn
        .query_row(
            "SELECT failed_runs
             FROM v_sync_daily_stats
             WHERE day='2026-02-26' AND job_type='notion_smoke'",
            [],
            |r| r.get(0),
        )
        .expect("query notion_smoke stats");
    assert_eq!(smoke_failed_runs, 1);
}
