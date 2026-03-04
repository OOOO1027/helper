use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sync_notion::notion_api::{NotionHttpClient, NotionPageSnapshot};
use crate::sync_notion::service::{
    sync_pending_page_tree_with_conn_with_options, sync_pending_with_conn_with_options,
    SyncRunSummary,
};
use crate::sync_notion::tree::clean_item_title;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionSmokeItem {
    pub sync_record_id: String,
    pub normalized_item_id: String,
    pub remote_found: bool,
    pub title_match: bool,
    pub summary_match: bool,
    pub source_match: bool,
    pub source_url_match: bool,
    pub overall_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionSmokeReport {
    pub mode: String,
    pub started_at: String,
    pub summary: SyncRunSummary,
    pub checked: u32,
    pub matched: u32,
    pub mismatched: u32,
    pub items: Vec<NotionSmokeItem>,
}

#[derive(Debug, Clone)]
struct LocalSyncSnapshot {
    sync_record_id: String,
    normalized_item_id: String,
    title: String,
    summary: String,
    source: String,
    source_url: Option<String>,
}

#[derive(Debug, Clone)]
struct LocalTreeSyncSnapshot {
    sync_record_id: String,
    normalized_item_id: String,
    title: String,
    summary: String,
    source: String,
    source_url: Option<String>,
    page_id: Option<String>,
    meta_block_id: Option<String>,
    summary_block_id: Option<String>,
}

pub async fn run_notion_sync_smoke_with_conn(
    conn: &Connection,
    client: &NotionHttpClient,
    limit: usize,
    check_top: usize,
    sleep_ms: u64,
) -> Result<NotionSmokeReport> {
    let started_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let summary = sync_pending_with_conn_with_options(conn, client, limit, sleep_ms).await?;
    let snapshots = load_recent_success_snapshots(conn, &started_at, check_top)?;

    let mut items = Vec::new();
    let mut matched = 0u32;
    let mut mismatched = 0u32;

    for snapshot in snapshots {
        let remote = client
            .query_page_snapshot_by_normalized_id(&snapshot.normalized_item_id)
            .await?;
        let item = compare_snapshot(&snapshot, remote.as_ref());
        if item.overall_match {
            matched += 1;
        } else {
            mismatched += 1;
        }
        items.push(item);
    }

    Ok(NotionSmokeReport {
        mode: "database".to_string(),
        started_at,
        summary,
        checked: items.len() as u32,
        matched,
        mismatched,
        items,
    })
}

pub async fn run_notion_page_tree_smoke_with_conn(
    conn: &Connection,
    client: &NotionHttpClient,
    root_page_id: &str,
    limit: usize,
    check_top: usize,
    sleep_ms: u64,
) -> Result<NotionSmokeReport> {
    let started_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let summary =
        sync_pending_page_tree_with_conn_with_options(conn, client, root_page_id, limit, sleep_ms)
            .await?;
    let snapshots = load_recent_success_tree_snapshots(conn, &started_at, check_top)?;

    let mut items = Vec::new();
    let mut matched = 0u32;
    let mut mismatched = 0u32;

    for snapshot in snapshots {
        let remote_title = if let Some(page_id) = snapshot.page_id.as_deref() {
            Some(client.get_page_title(page_id).await?)
        } else {
            None
        };
        let meta_text = if let Some(block_id) = snapshot.meta_block_id.as_deref() {
            Some(client.get_paragraph_block_text(block_id).await?)
        } else {
            None
        };
        let summary_text = if let Some(block_id) = snapshot.summary_block_id.as_deref() {
            Some(client.get_paragraph_block_text(block_id).await?)
        } else {
            None
        };

        let item = compare_tree_snapshot(
            &snapshot,
            remote_title.as_deref(),
            meta_text.as_deref(),
            summary_text.as_deref(),
        );
        if item.overall_match {
            matched += 1;
        } else {
            mismatched += 1;
        }
        items.push(item);
    }

    Ok(NotionSmokeReport {
        mode: "page_tree".to_string(),
        started_at,
        summary,
        checked: items.len() as u32,
        matched,
        mismatched,
        items,
    })
}

pub fn persist_smoke_report(conn: &Connection, report: &NotionSmokeReport) -> Result<String> {
    let finished_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let fail_count = report.summary.failed + report.mismatched;
    let status = if report.summary.attempted == 0 {
        "skipped"
    } else if fail_count > 0 {
        "partial"
    } else {
        "success"
    };
    let (error_code, error_message): (Option<&str>, Option<String>) = if report.mismatched > 0 {
        (
            Some("INT-9000"),
            Some(format!(
                "notion smoke mismatched count: {}",
                report.mismatched
            )),
        )
    } else if report.summary.failed > 0 {
        (
            Some("MOD-3002"),
            Some(format!(
                "notion sync failed count: {}",
                report.summary.failed
            )),
        )
    } else {
        (None, None)
    };
    let metadata_json = serde_json::to_string(report)
        .map_err(|e| crate::BackendError::Internal(format!("encode smoke metadata failed: {e}")))?;

    let job_run_id = format!("jr_{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO job_runs(
            id, job_type, source, status, started_at, finished_at, success_count, fail_count,
            metadata_json, error_code, error_message, created_at
         ) VALUES (?1, 'notion_smoke', 'notion', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?4)",
        params![
            &job_run_id,
            status,
            &report.started_at,
            &finished_at,
            report.summary.succeeded as i64,
            fail_count as i64,
            metadata_json,
            error_code,
            error_message
        ],
    )?;
    Ok(job_run_id)
}

fn load_recent_success_snapshots(
    conn: &Connection,
    since: &str,
    limit: usize,
) -> Result<Vec<LocalSyncSnapshot>> {
    let mut stmt = conn.prepare(
        "SELECT
            s.id,
            n.id,
            IFNULL(src.title, ''),
            IFNULL(a.summary_text, n.text_clean),
            IFNULL(src.source, 'wechat'),
            src.source_url
         FROM sync_records s
         JOIN normalized_items n ON n.id = s.normalized_item_id
         LEFT JOIN source_items src ON src.id = n.source_item_id
         LEFT JOIN analysis_results a ON a.normalized_item_id = n.id
         WHERE s.target = 'notion'
           AND s.sync_state = 'success'
           AND s.updated_at >= ?1
         ORDER BY s.updated_at DESC
         LIMIT ?2",
    )?;

    let mapped = stmt.query_map(params![since, limit as i64], |row| {
        Ok(LocalSyncSnapshot {
            sync_record_id: row.get(0)?,
            normalized_item_id: row.get(1)?,
            title: row.get(2)?,
            summary: row.get(3)?,
            source: row.get(4)?,
            source_url: row.get(5)?,
        })
    })?;

    let mut out = Vec::new();
    for item in mapped {
        out.push(item?);
    }
    Ok(out)
}

fn load_recent_success_tree_snapshots(
    conn: &Connection,
    since: &str,
    limit: usize,
) -> Result<Vec<LocalTreeSyncSnapshot>> {
    let mut stmt = conn.prepare(
        "SELECT
            s.id,
            n.id,
            IFNULL(src.title, ''),
            IFNULL(a.summary_text, n.text_clean),
            IFNULL(src.source, 'wechat'),
            src.source_url,
            s.target_record_id,
            t.meta_block_id,
            t.summary_block_id
         FROM sync_records s
         JOIN normalized_items n ON n.id = s.normalized_item_id
         LEFT JOIN source_items src ON src.id = n.source_item_id
         LEFT JOIN analysis_results a ON a.normalized_item_id = n.id
         LEFT JOIN notion_tree_nodes t ON t.normalized_item_id = n.id AND t.node_type='item'
         WHERE s.target = 'notion'
           AND s.sync_state = 'success'
           AND s.updated_at >= ?1
         ORDER BY s.updated_at DESC
         LIMIT ?2",
    )?;

    let mapped = stmt.query_map(params![since, limit as i64], |row| {
        Ok(LocalTreeSyncSnapshot {
            sync_record_id: row.get(0)?,
            normalized_item_id: row.get(1)?,
            title: row.get(2)?,
            summary: row.get(3)?,
            source: row.get(4)?,
            source_url: row.get(5)?,
            page_id: row.get(6)?,
            meta_block_id: row.get(7)?,
            summary_block_id: row.get(8)?,
        })
    })?;

    let mut out = Vec::new();
    for item in mapped {
        out.push(item?);
    }
    Ok(out)
}

fn compare_snapshot(
    local: &LocalSyncSnapshot,
    remote: Option<&NotionPageSnapshot>,
) -> NotionSmokeItem {
    let Some(remote) = remote else {
        return NotionSmokeItem {
            sync_record_id: local.sync_record_id.clone(),
            normalized_item_id: local.normalized_item_id.clone(),
            remote_found: false,
            title_match: false,
            summary_match: false,
            source_match: false,
            source_url_match: false,
            overall_match: false,
        };
    };

    let title_match = text_normalize(&local.title) == text_normalize(&remote.title);
    let summary_match = text_normalize(&local.summary) == text_normalize(&remote.summary);
    let source_match = local.source.trim() == remote.source.trim();
    let source_url_match = normalize_url_opt(local.source_url.as_deref())
        == normalize_url_opt(remote.source_url.as_deref());
    let normalized_id_match = local.normalized_item_id.trim() == remote.normalized_item_id.trim();
    let overall_match =
        title_match && summary_match && source_match && source_url_match && normalized_id_match;

    NotionSmokeItem {
        sync_record_id: local.sync_record_id.clone(),
        normalized_item_id: local.normalized_item_id.clone(),
        remote_found: true,
        title_match,
        summary_match,
        source_match,
        source_url_match,
        overall_match,
    }
}

fn compare_tree_snapshot(
    local: &LocalTreeSyncSnapshot,
    remote_title: Option<&str>,
    remote_meta: Option<&str>,
    remote_summary: Option<&str>,
) -> NotionSmokeItem {
    let Some(title) = remote_title else {
        return NotionSmokeItem {
            sync_record_id: local.sync_record_id.clone(),
            normalized_item_id: local.normalized_item_id.clone(),
            remote_found: false,
            title_match: false,
            summary_match: false,
            source_match: false,
            source_url_match: false,
            overall_match: false,
        };
    };
    let meta = remote_meta.unwrap_or_default();
    let summary = remote_summary.unwrap_or_default();
    let expected_title = clean_item_title(&local.title);
    let title_match = text_normalize(&expected_title) == text_normalize(title);
    let local_summary = local.summary.trim();
    let summary_match = text_normalize(local_summary) == text_normalize(summary)
        || (!local_summary.is_empty() && summary.contains(local_summary));
    let source_match = meta.contains(&format!("source: {}", local.source));
    let expected_url = local.source_url.as_deref().map(str::trim).unwrap_or("-");
    let source_url_match = meta.contains(&format!("url: {}", expected_url));
    let normalized_id_match =
        meta.contains(&format!("normalized_id: {}", local.normalized_item_id));
    let overall_match =
        title_match && summary_match && source_match && source_url_match && normalized_id_match;

    NotionSmokeItem {
        sync_record_id: local.sync_record_id.clone(),
        normalized_item_id: local.normalized_item_id.clone(),
        remote_found: true,
        title_match,
        summary_match,
        source_match,
        source_url_match,
        overall_match,
    }
}

fn text_normalize(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_url_opt(input: Option<&str>) -> Option<String> {
    input.map(|v| {
        let mut s = v.trim().to_string();
        while s.ends_with('/') && s.len() > "https://".len() {
            s.pop();
        }
        s
    })
}

#[cfg(test)]
mod tests {
    use super::{
        compare_snapshot, compare_tree_snapshot, persist_smoke_report, LocalSyncSnapshot,
        LocalTreeSyncSnapshot, NotionPageSnapshot, NotionSmokeReport, SyncRunSummary,
    };
    use crate::storage::db::run_migrations;
    use rusqlite::Connection;

    #[test]
    fn compare_snapshot_accepts_whitespace_and_trailing_slash_differences() {
        let local = LocalSyncSnapshot {
            sync_record_id: "sync_1".to_string(),
            normalized_item_id: "norm_1".to_string(),
            title: "hello   world".to_string(),
            summary: "a  b c".to_string(),
            source: "wechat".to_string(),
            source_url: Some("https://example.com/a/".to_string()),
        };
        let remote = NotionPageSnapshot {
            page_id: "pg_1".to_string(),
            normalized_item_id: "norm_1".to_string(),
            title: "hello world".to_string(),
            summary: "a b c".to_string(),
            source: "wechat".to_string(),
            source_url: Some("https://example.com/a".to_string()),
        };
        let item = compare_snapshot(&local, Some(&remote));
        assert!(item.overall_match);
    }

    #[test]
    fn compare_snapshot_flags_missing_remote() {
        let local = LocalSyncSnapshot {
            sync_record_id: "sync_1".to_string(),
            normalized_item_id: "norm_1".to_string(),
            title: "t".to_string(),
            summary: "s".to_string(),
            source: "wechat".to_string(),
            source_url: None,
        };
        let item = compare_snapshot(&local, None);
        assert!(!item.remote_found);
        assert!(!item.overall_match);
    }

    #[test]
    fn persist_smoke_report_records_partial_job_run() {
        let conn = Connection::open_in_memory().expect("open memory");
        run_migrations(&conn).expect("migrations");

        let report = NotionSmokeReport {
            mode: "database".to_string(),
            started_at: "2026-02-26 00:00:00".to_string(),
            summary: SyncRunSummary {
                scanned: 2,
                attempted: 2,
                succeeded: 1,
                failed: 1,
                requeued: 0,
                dead_lettered: 0,
            },
            checked: 1,
            matched: 0,
            mismatched: 1,
            items: vec![],
        };
        let job_run_id = persist_smoke_report(&conn, &report).expect("persist report");
        let (status, success_count, fail_count): (String, i64, i64) = conn
            .query_row(
                "SELECT status, success_count, fail_count FROM job_runs WHERE id = ?1",
                [job_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query job_run");

        assert_eq!(status, "partial");
        assert_eq!(success_count, 1);
        assert_eq!(fail_count, 2);
    }

    #[test]
    fn compare_tree_snapshot_matches_metadata_and_summary() {
        let local = LocalTreeSyncSnapshot {
            sync_record_id: "sync_1".to_string(),
            normalized_item_id: "norm_1".to_string(),
            title: "hello world".to_string(),
            summary: "summary body".to_string(),
            source: "wechat".to_string(),
            source_url: Some("https://example.com/a".to_string()),
            page_id: Some("pg_1".to_string()),
            meta_block_id: Some("blk_meta".to_string()),
            summary_block_id: Some("blk_sum".to_string()),
        };
        let item = compare_tree_snapshot(
            &local,
            Some("hello world"),
            Some("source: wechat | url: https://example.com/a | normalized_id: norm_1"),
            Some("summary body"),
        );
        assert!(item.overall_match);
    }
}
