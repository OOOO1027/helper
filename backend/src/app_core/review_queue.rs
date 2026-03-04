use chrono::Local;
use rusqlite::{params, Connection};

use crate::{BackendError, Result};

use super::adapters::sqlite_core_port::SqliteCoreDataPort;
use super::ports::CoreDataPort;
use super::{AppCore, PageReq, Paged, ReviewFilter, ReviewItem, ReviewPatch};

impl AppCore {
    pub fn get_review_items_with_conn(
        &self,
        conn: &Connection,
        filter: ReviewFilter,
        page: PageReq,
    ) -> Result<Paged<ReviewItem>> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let _ = self.apply_xhs_auto_pass_policy(conn, &now)?;
        if page.page == 0 || page.page_size == 0 {
            return Err(BackendError::Validation(
                "page and page_size must be positive".to_string(),
            ));
        }
        let state_filter = super::normalize_review_state_filter(filter.state)?;
        let port = SqliteCoreDataPort::new(conn);
        port.list_review_items(state_filter.as_deref(), filter.min_priority, page)
    }

    pub fn update_review_item_with_conn(
        &self,
        conn: &Connection,
        id: &str,
        patch: ReviewPatch,
    ) -> Result<ReviewItem> {
        if id.trim().is_empty() {
            return Err(BackendError::Validation("id must not be empty".to_string()));
        }
        let state_filter = super::normalize_review_state_filter(patch.state)?;
        let note = super::normalize_optional_filter(patch.note);
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let port = SqliteCoreDataPort::new(conn);

        if state_filter.is_none() && note.is_none() {
            return port.get_review_item(id);
        }

        let changed =
            port.update_review_item(id, state_filter.as_deref(), note.as_deref(), &now)?;
        if !changed {
            return Err(BackendError::Validation(format!(
                "review item not found: {id}"
            )));
        }
        let item = port.get_review_item(id)?;
        if let Some(state) = state_filter.as_deref() {
            self.sync_publish_state_for_review_decision(
                conn,
                &item.normalized_item_id,
                state,
                &now,
            )?;
        }
        port.get_review_item(id)
    }

    fn sync_publish_state_for_review_decision(
        &self,
        conn: &Connection,
        normalized_item_id: &str,
        review_state: &str,
        now: &str,
    ) -> Result<()> {
        match review_state {
            "done" => {
                conn.execute(
                    "UPDATE sync_records
                     SET sync_state='pending',
                         retry_count=0,
                         next_retry_at=NULL,
                         last_error_code=NULL,
                         last_error_message=NULL,
                         updated_at=?2
                     WHERE target='notion'
                       AND normalized_item_id=?1
                       AND sync_state IN ('pending','retry','failed')",
                    params![normalized_item_id, now],
                )?;
                conn.execute(
                    "INSERT INTO publish_tasks(
                        id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
                     )
                     SELECT
                        ('pt_' || n.source_item_id) AS id,
                        n.source_item_id AS item_id,
                        CASE
                          WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'ignored'
                          ELSE 'pending'
                        END AS state,
                        0 AS attempt_count,
                        NULL AS next_retry_at,
                        CASE
                          WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source_inactive'
                          ELSE NULL
                        END AS error_code,
                        CASE
                          WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source marked inactive'
                          ELSE NULL
                        END AS error_message,
                        ?2 AS created_at,
                        ?2 AS updated_at
                     FROM normalized_items n
                     LEFT JOIN source_state ss ON ss.item_id = n.source_item_id
                     WHERE n.id = ?1
                     ON CONFLICT(id) DO UPDATE SET
                        state=excluded.state,
                        attempt_count=excluded.attempt_count,
                        next_retry_at=excluded.next_retry_at,
                        error_code=excluded.error_code,
                        error_message=excluded.error_message,
                        updated_at=excluded.updated_at",
                    params![normalized_item_id, now],
                )?;
            }
            "rejected" => {
                conn.execute(
                    "UPDATE sync_records
                     SET sync_state='failed',
                         retry_count=retry_count + 1,
                         next_retry_at=NULL,
                         last_error_code='review_rejected',
                         last_error_message='manual review rejected',
                         updated_at=?2
                     WHERE target='notion'
                       AND normalized_item_id=?1
                       AND sync_state IN ('pending','retry')",
                    params![normalized_item_id, now],
                )?;
                conn.execute(
                    "INSERT INTO publish_tasks(
                        id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
                     )
                     SELECT
                        ('pt_' || n.source_item_id) AS id,
                        n.source_item_id AS item_id,
                        'ignored' AS state,
                        0 AS attempt_count,
                        NULL AS next_retry_at,
                        'review_rejected' AS error_code,
                        'manual review rejected' AS error_message,
                        ?2 AS created_at,
                        ?2 AS updated_at
                     FROM normalized_items n
                     WHERE n.id = ?1
                     ON CONFLICT(id) DO UPDATE SET
                        state='ignored',
                        attempt_count=0,
                        next_retry_at=NULL,
                        error_code='review_rejected',
                        error_message='manual review rejected',
                        updated_at=excluded.updated_at",
                    params![normalized_item_id, now],
                )?;
            }
            _ => {}
        }
        Ok(())
    }
}
