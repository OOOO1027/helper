use rusqlite::{params, Connection};

use crate::app_core::ports::{CoreDataPort, DashboardSnapshot};
use crate::app_core::{PageReq, Paged, ReviewItem};
use crate::{BackendError, Result};

pub struct SqliteCoreDataPort<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteCoreDataPort<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

impl CoreDataPort for SqliteCoreDataPort<'_> {
    fn collect_counts(
        &self,
        source: &str,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<(u32, u32)> {
        let since = since.map(|v| v.trim()).filter(|v| !v.is_empty());
        let until = until.map(|v| v.trim()).filter(|v| !v.is_empty());
        let fetched: i64 = self.conn.query_row(
            "SELECT COUNT(1)
             FROM source_items
             WHERE source = ?1
               AND (?2 IS NULL OR collected_at >= ?2)
               AND (?3 IS NULL OR collected_at <= ?3)",
            params![source, since, until],
            |r| r.get(0),
        )?;
        let stored: i64 = self.conn.query_row(
            "SELECT COUNT(1)
             FROM source_items
             WHERE source = ?1
               AND status != 'deleted'
               AND (?2 IS NULL OR collected_at >= ?2)
               AND (?3 IS NULL OR collected_at <= ?3)",
            params![source, since, until],
            |r| r.get(0),
        )?;
        Ok((fetched.max(0) as u32, stored.max(0) as u32))
    }

    fn dashboard_snapshot(&self, today: &str, month: &str) -> Result<DashboardSnapshot> {
        let today_collected: i64 = self.conn.query_row(
            "SELECT COUNT(1)
             FROM source_items
             WHERE substr(collected_at, 1, 10) = ?1",
            params![today],
            |r| r.get(0),
        )?;
        let pending_review: i64 = self.conn.query_row(
            "SELECT COUNT(1) FROM review_queue WHERE state = 'pending'",
            [],
            |r| r.get(0),
        )?;
        let classification_accuracy: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(AVG(c_score), 0.0) FROM analysis_results",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);
        let summary_usability: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(AVG(s_score), 0.0) FROM analysis_results",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);
        let used_from_ledger: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(cost_cny), 0.0)
                 FROM budget_ledger
                 WHERE day LIKE (?1 || '%')",
                params![month],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        Ok(DashboardSnapshot {
            today_collected: today_collected.max(0) as u32,
            pending_review: pending_review.max(0) as u32,
            classification_accuracy,
            summary_usability,
            used_from_ledger,
        })
    }

    fn list_review_items(
        &self,
        state: Option<&str>,
        min_priority: Option<f64>,
        page: PageReq,
    ) -> Result<Paged<ReviewItem>> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(1)
             FROM review_queue
             WHERE (?1 IS NULL OR state = ?1)
               AND (?2 IS NULL OR priority >= ?2)",
            params![state, min_priority],
            |r| r.get(0),
        )?;

        let page_size = page.page_size as i64;
        let offset = ((page.page - 1) * page.page_size) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT q.id,
                    q.normalized_item_id,
                    q.priority,
                    q.reason,
                    q.state,
                    COALESCE(
                      NULLIF(TRIM(s.title), ''),
                      NULLIF(TRIM(substr(s.content_raw, 1, 120)), '')
                    ) AS title,
                    COALESCE(
                      NULLIF(TRIM(s.source_url), ''),
                      NULLIF(TRIM(n.canonical_url), '')
                    ) AS url,
                    CASE
                      WHEN n.quality_score >= 85 THEN 'high'
                      WHEN n.quality_score >= 65 THEN 'mid'
                      WHEN n.quality_score >= 0 THEN 'low'
                      ELSE 'unknown'
                    END AS quality_band,
                    CASE COALESCE(sr.sync_state, 'pending')
                      WHEN 'success' THEN 'published'
                      WHEN 'failed' THEN 'failed'
                      WHEN 'retry' THEN 'retry'
                      ELSE 'pending'
                    END AS publish_state
             FROM review_queue q
             LEFT JOIN normalized_items n ON n.id = q.normalized_item_id
             LEFT JOIN source_items s ON s.id = n.source_item_id
             LEFT JOIN sync_records sr ON sr.normalized_item_id = q.normalized_item_id AND sr.target = 'notion'
             WHERE (?1 IS NULL OR q.state = ?1)
               AND (?2 IS NULL OR q.priority >= ?2)
             ORDER BY q.priority DESC, q.created_at ASC
             LIMIT ?3 OFFSET ?4",
        )?;
        let mapped = stmt.query_map(params![state, min_priority, page_size, offset], |row| {
            Ok(ReviewItem {
                id: row.get(0)?,
                normalized_item_id: row.get(1)?,
                priority: row.get::<_, f64>(2)?,
                reason: row.get(3)?,
                state: row.get(4)?,
                title: normalize_optional_text(row.get(5)?),
                url: normalize_optional_text(row.get(6)?),
                quality_band: row.get(7)?,
                publish_state: row.get(8)?,
            })
        })?;

        let mut items = Vec::new();
        for item in mapped {
            items.push(item?);
        }
        Ok(Paged {
            items,
            page: page.page,
            page_size: page.page_size,
            total: total.max(0) as u64,
        })
    }

    fn get_review_item(&self, id: &str) -> Result<ReviewItem> {
        self.conn
            .query_row(
                "SELECT q.id,
                        q.normalized_item_id,
                        q.priority,
                        q.reason,
                        q.state,
                        COALESCE(
                          NULLIF(TRIM(s.title), ''),
                          NULLIF(TRIM(substr(s.content_raw, 1, 120)), '')
                        ) AS title,
                        COALESCE(
                          NULLIF(TRIM(s.source_url), ''),
                          NULLIF(TRIM(n.canonical_url), '')
                        ) AS url,
                        CASE
                          WHEN n.quality_score >= 85 THEN 'high'
                          WHEN n.quality_score >= 65 THEN 'mid'
                          WHEN n.quality_score >= 0 THEN 'low'
                          ELSE 'unknown'
                        END AS quality_band,
                        CASE COALESCE(sr.sync_state, 'pending')
                          WHEN 'success' THEN 'published'
                          WHEN 'failed' THEN 'failed'
                          WHEN 'retry' THEN 'retry'
                          ELSE 'pending'
                        END AS publish_state
                 FROM review_queue q
                 LEFT JOIN normalized_items n ON n.id = q.normalized_item_id
                 LEFT JOIN source_items s ON s.id = n.source_item_id
                 LEFT JOIN sync_records sr ON sr.normalized_item_id = q.normalized_item_id AND sr.target = 'notion'
                 WHERE q.id = ?1",
                params![id],
                |row| {
                    Ok(ReviewItem {
                        id: row.get(0)?,
                        normalized_item_id: row.get(1)?,
                        priority: row.get(2)?,
                        reason: row.get(3)?,
                        state: row.get(4)?,
                        title: normalize_optional_text(row.get(5)?),
                        url: normalize_optional_text(row.get(6)?),
                        quality_band: row.get(7)?,
                        publish_state: row.get(8)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    BackendError::Validation(format!("review item not found: {id}"))
                }
                other => BackendError::Storage(other),
            })
    }

    fn update_review_item(
        &self,
        id: &str,
        state: Option<&str>,
        reason: Option<&str>,
        updated_at: &str,
    ) -> Result<bool> {
        let changed = self.conn.execute(
            "UPDATE review_queue
             SET state = COALESCE(?2, state),
                 reason = COALESCE(?3, reason),
                 updated_at = ?4
             WHERE id = ?1",
            params![id, state, reason, updated_at],
        )?;
        Ok(changed > 0)
    }

    fn retry_review_items(&self, ids: &[String], updated_at: &str) -> Result<(u32, u32)> {
        let mut requeued = 0u32;
        let mut ignored = 0u32;
        for id in ids {
            if id.trim().is_empty() {
                ignored += 1;
                continue;
            }
            let changed = self.conn.execute(
                "UPDATE review_queue
                 SET state = 'pending',
                     updated_at = ?2
                 WHERE id = ?1
                   AND state != 'pending'",
                params![id, updated_at],
            )?;
            if changed > 0 {
                requeued += 1;
            } else {
                ignored += 1;
            }
        }
        Ok((requeued, ignored))
    }
}
