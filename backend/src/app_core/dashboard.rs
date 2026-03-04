use chrono::Local;
use rusqlite::{params, Connection};

use crate::Result;

use super::adapters::sqlite_core_port::SqliteCoreDataPort;
use super::ports::CoreDataPort;
use super::{
    AppCore, CollectionInsight, CollectionRecentItem, CollectionSourceBreakdown, DashboardMetrics,
};

impl AppCore {
    pub fn get_dashboard_metrics_with_conn(&self, conn: &Connection) -> Result<DashboardMetrics> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let _ = self.apply_xhs_auto_pass_policy(conn, &now)?;
        let today = Local::now().format("%Y-%m-%d").to_string();
        let month = Local::now().format("%Y-%m").to_string();

        let port = SqliteCoreDataPort::new(conn);
        let snapshot = port.dashboard_snapshot(&today, &month)?;
        let used_cny = if snapshot.used_from_ledger > 0.0 {
            snapshot.used_from_ledger
        } else {
            self.budget.used_cny
        };
        let budget_usage_ratio = if self.budget.limit_cny <= 0.0 {
            0.0
        } else {
            used_cny / self.budget.limit_cny
        };

        Ok(DashboardMetrics {
            today_collected: snapshot.today_collected,
            pending_review: snapshot.pending_review,
            classification_accuracy: snapshot.classification_accuracy,
            summary_usability: snapshot.summary_usability,
            budget_usage_ratio,
        })
    }

    pub fn get_collection_insight_with_conn(&self, conn: &Connection) -> Result<CollectionInsight> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let _ = self.apply_xhs_auto_pass_policy(conn, &now)?;
        let today = Local::now().format("%Y-%m-%d").to_string();

        let today_total: i64 = conn.query_row(
            "SELECT COUNT(1)
             FROM source_items
             WHERE substr(collected_at, 1, 10) = ?1",
            params![&today],
            |r| r.get(0),
        )?;

        let mut source_stmt = conn.prepare(
            "SELECT source, COUNT(1) AS c
             FROM source_items
             WHERE substr(collected_at, 1, 10) = ?1
             GROUP BY source
             ORDER BY c DESC, source ASC",
        )?;
        let source_rows = source_stmt.query_map(params![&today], |row| {
            Ok(CollectionSourceBreakdown {
                source: row.get(0)?,
                count: row.get::<_, i64>(1)?.max(0) as u32,
            })
        })?;
        let mut sources = Vec::new();
        for row in source_rows {
            sources.push(row?);
        }

        let mut review_pending = 0u32;
        let mut review_done = 0u32;
        let mut review_rejected = 0u32;
        let mut direct_no_review = 0u32;
        let mut review_stmt = conn.prepare(
            "SELECT COALESCE(q.state, 'not_in_queue') AS state, COUNT(1)
             FROM source_items s
             LEFT JOIN normalized_items n ON n.source_item_id = s.id
             LEFT JOIN review_queue q ON q.id = (
                SELECT rq2.id
                FROM review_queue rq2
                WHERE rq2.normalized_item_id = n.id
                ORDER BY rq2.updated_at DESC
                LIMIT 1
             )
             WHERE substr(s.collected_at, 1, 10) = ?1
             GROUP BY COALESCE(q.state, 'not_in_queue')",
        )?;
        let review_rows = review_stmt.query_map(params![&today], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?.max(0) as u32,
            ))
        })?;
        for row in review_rows {
            let (state, count) = row?;
            match state.as_str() {
                "pending" | "processing" => review_pending += count,
                "done" => review_done += count,
                "rejected" => review_rejected += count,
                _ => direct_no_review += count,
            }
        }

        let mut sync_pending = 0u32;
        let mut sync_success = 0u32;
        let mut sync_failed = 0u32;
        let mut sync_retry = 0u32;
        let mut sync_not_started = 0u32;
        let mut sync_stmt = conn.prepare(
            "SELECT COALESCE(sr.sync_state, 'not_started') AS sync_state, COUNT(1)
             FROM source_items s
             LEFT JOIN normalized_items n ON n.source_item_id = s.id
             LEFT JOIN sync_records sr
               ON sr.normalized_item_id = n.id
              AND sr.target = 'notion'
             WHERE substr(s.collected_at, 1, 10) = ?1
             GROUP BY COALESCE(sr.sync_state, 'not_started')",
        )?;
        let sync_rows = sync_stmt.query_map(params![&today], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?.max(0) as u32,
            ))
        })?;
        for row in sync_rows {
            let (state, count) = row?;
            match state.as_str() {
                "pending" => sync_pending += count,
                "success" => sync_success += count,
                "failed" => sync_failed += count,
                "retry" => sync_retry += count,
                _ => sync_not_started += count,
            }
        }

        let mut recent_stmt = conn.prepare(
            "SELECT s.source,
                    COALESCE(NULLIF(TRIM(s.title), ''), NULLIF(TRIM(substr(s.content_raw, 1, 56)), ''), s.id) AS title,
                    s.collected_at,
                    COALESCE(q.state, 'not_in_queue') AS review_state,
                    COALESCE(sr.sync_state, 'not_started') AS sync_state
             FROM source_items s
             LEFT JOIN normalized_items n ON n.source_item_id = s.id
             LEFT JOIN review_queue q ON q.id = (
                SELECT rq2.id
                FROM review_queue rq2
                WHERE rq2.normalized_item_id = n.id
                ORDER BY rq2.updated_at DESC
                LIMIT 1
             )
             LEFT JOIN sync_records sr
               ON sr.normalized_item_id = n.id
              AND sr.target = 'notion'
             WHERE substr(s.collected_at, 1, 10) = ?1
             ORDER BY s.collected_at DESC
             LIMIT 12",
        )?;
        let recent_rows = recent_stmt.query_map(params![&today], |row| {
            Ok(CollectionRecentItem {
                source: row.get(0)?,
                title: row.get::<_, String>(1)?,
                collected_at: row.get(2)?,
                review_state: row.get(3)?,
                sync_state: row.get(4)?,
            })
        })?;
        let mut recent_items = Vec::new();
        for row in recent_rows {
            recent_items.push(row?);
        }

        Ok(CollectionInsight {
            day: today,
            today_total: today_total.max(0) as u32,
            sources,
            review_pending,
            review_done,
            review_rejected,
            direct_no_review,
            sync_pending,
            sync_success,
            sync_failed,
            sync_retry,
            sync_not_started,
            recent_items,
        })
    }
}
