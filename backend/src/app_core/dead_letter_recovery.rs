use chrono::Local;
use rusqlite::{params, Connection};

use crate::collectors::wechat_import::parse_wechat_files;
use crate::Result;

use super::adapters::sqlite_core_port::SqliteCoreDataPort;
use super::ports::CoreDataPort;
use super::{AppCore, PipelinePreviewReq, RetryResult};

impl AppCore {
    pub fn retry_failed_items_with_conn(
        &self,
        conn: &Connection,
        ids: &[String],
    ) -> Result<RetryResult> {
        if ids.is_empty() {
            return Ok(RetryResult {
                requested: 0,
                requeued: 0,
                ignored: 0,
            });
        }
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let port = SqliteCoreDataPort::new(conn);
        let (requeued, ignored) = port.retry_review_items(ids, &now)?;
        Ok(RetryResult {
            requested: ids.len() as u32,
            requeued,
            ignored,
        })
    }

    pub fn retry_dead_letters_with_conn(
        &self,
        conn: &mut Connection,
        dead_letter_ids: &[String],
    ) -> Result<RetryResult> {
        if dead_letter_ids.is_empty() {
            return Ok(RetryResult {
                requested: 0,
                requeued: 0,
                ignored: 0,
            });
        }
        let mut requeued = 0u32;
        let mut ignored = 0u32;
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let replay_by = std::env::var("HELPER_OPERATOR").unwrap_or_else(|_| "system".to_string());

        for id in dead_letter_ids {
            let started = std::time::Instant::now();
            let row = conn.query_row(
                "SELECT entity_type, entity_id, stage, state, payload_json FROM dead_letter WHERE id = ?1",
                params![id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                },
            );
            let (entity_type, entity_id, stage, state, payload_json) = match row {
                Ok(v) => v,
                Err(_) => {
                    ignored += 1;
                    continue;
                }
            };
            if state != "open" {
                ignored += 1;
                continue;
            }

            let mut resolved = false;

            if entity_type == "sync_record"
                && (stage == "notion_sync" || stage == "notion_sync_page_tree")
            {
                let updated = conn.execute(
                    "UPDATE sync_records
                     SET sync_state='retry',
                         retry_count=CASE WHEN retry_count > 0 THEN retry_count - 1 ELSE 0 END,
                         last_error_code=NULL,
                         last_error_message=NULL,
                         next_retry_at=NULL,
                         updated_at=?2
                     WHERE id=?1",
                    params![entity_id, now],
                )?;
                if updated > 0 {
                    conn.execute(
                        "UPDATE publish_tasks
                         SET state='pending',
                             next_retry_at=NULL,
                             error_code=NULL,
                             error_message=NULL,
                             updated_at=?2
                         WHERE item_id = (
                           SELECT n.source_item_id
                           FROM sync_records s
                           JOIN normalized_items n ON n.id = s.normalized_item_id
                           WHERE s.id = ?1
                         )",
                        params![entity_id, now],
                    )?;
                }
                resolved = updated > 0;
            } else if entity_type == "pipeline"
                && (stage == "wechat_import_parse"
                    || stage == "wechat_persist"
                    || stage == "pipeline_execute")
            {
                resolved =
                    self.replay_pipeline_dead_letter(conn, &stage, &entity_id, &payload_json)?;
            }

            if resolved {
                let latency_ms = started.elapsed().as_millis() as i64;
                conn.execute(
                    "UPDATE dead_letter
                     SET state='resolved',
                         resolved_at=?2,
                         replay_count=COALESCE(replay_count,0)+1,
                         last_replayed_at=?2,
                         last_replay_status='resolved',
                         last_replayed_by=?3,
                         last_replay_latency_ms=?4
                     WHERE id=?1",
                    params![id, now, &replay_by, latency_ms],
                )?;
                requeued += 1;
            } else {
                let latency_ms = started.elapsed().as_millis() as i64;
                let _ = conn.execute(
                    "UPDATE dead_letter
                     SET replay_count=COALESCE(replay_count,0)+1,
                         last_replayed_at=?2,
                         last_replay_status='ignored',
                         last_replayed_by=?3,
                         last_replay_latency_ms=?4
                     WHERE id=?1",
                    params![id, now, &replay_by, latency_ms],
                );
                ignored += 1;
            }
        }

        Ok(RetryResult {
            requested: dead_letter_ids.len() as u32,
            requeued,
            ignored,
        })
    }

    fn replay_pipeline_dead_letter(
        &self,
        conn: &mut Connection,
        stage: &str,
        entity_id: &str,
        payload_json: &str,
    ) -> Result<bool> {
        if stage == "pipeline_execute" {
            if let Ok(req) = serde_json::from_str::<PipelinePreviewReq>(payload_json) {
                return Ok(self.execute_pipeline_with_conn(conn, req).is_ok());
            }
        }

        let path = if !entity_id.trim().is_empty() && std::path::Path::new(entity_id).exists() {
            Some(entity_id.to_string())
        } else {
            serde_json::from_str::<serde_json::Value>(payload_json)
                .ok()
                .and_then(|v| {
                    v.get("path")
                        .and_then(|p| p.as_str())
                        .map(|s| s.to_string())
                })
                .filter(|p| std::path::Path::new(p).exists())
        };
        let Some(path) = path else {
            return Ok(false);
        };

        let parsed = parse_wechat_files(&[path]);
        if parsed.items.is_empty() {
            return Ok(false);
        }
        let mut persisted = 0u32;
        for item in parsed.items {
            let req = PipelinePreviewReq {
                source: "wechat".to_string(),
                source_url: item.source_url.clone(),
                published_at: item.published_at.clone(),
                content_raw: item.content_raw.clone(),
            };
            if self.execute_pipeline_with_conn(conn, req).is_ok() {
                persisted += 1;
            }
        }
        Ok(persisted > 0)
    }
}
