use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use tracing::info;

use crate::collectors::xhs::XhsCollector;
use crate::collectors::CollectWindow;
use crate::pipeline::normalize::{normalize, NormalizeInput};
use crate::pipeline::score::should_trigger_review;
use crate::{BackendError, Result};

use super::{AppCore, CollectResult, PipelinePreviewReq};

const XHS_REASON_MANUAL_REVIEW: &str = "XHS anomaly requires manual review";
const XHS_REASON_AUTO_PASS: &str = "XHS auto-pass normal item";
const XHS_MIN_CONTENT_CHARS: i64 = 4;

pub(super) fn xhs_requires_manual_review(
    analyzed: &crate::pipeline::analyze::AnalyzeOutput,
    text_clean: &str,
) -> bool {
    let content_len = text_clean.chars().count() as i64;
    let too_short = content_len < XHS_MIN_CONTENT_CHARS;
    let low_confidence =
        analyzed.classification_confidence < 0.70 || analyzed.summary_confidence < 0.60;
    let low_quality = analyzed.quality_score < 65.0;
    analyzed.requires_review || too_short || low_confidence || low_quality
}

impl AppCore {
    pub(super) fn collect_xhs_with_conn(
        &self,
        conn: &mut Connection,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<CollectResult> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let (auto_done, kept_pending) = self.apply_xhs_auto_pass_policy(conn, &now)?;
        if auto_done > 0 || kept_pending > 0 {
            info!(
                source = "xhs",
                auto_done, kept_pending, "xhs policy applied before collection"
            );
        }
        let known_fingerprints = self.load_known_xhs_fingerprints(conn)?;
        let mut seen_fingerprints = known_fingerprints;
        let collector = XhsCollector::default();
        let window = CollectWindow {
            since: since.map(|v| v.to_string()),
            until: until.map(|v| v.to_string()),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| BackendError::Internal(format!("build tokio runtime failed: {e}")))?;
        let collected = runtime.block_on(collector.collect_internal(&window, &HashSet::new()))?;
        let fetched = collected.len() as u32;
        let mut stored = 0u32;
        let mut skipped_existing = 0u32;
        let mut backfilled_existing = 0u32;

        for item in collected {
            let collected_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let normalized = normalize(&NormalizeInput {
                source: "xhs".to_string(),
                source_url: item.source_url.clone(),
                published_at: item.published_at.clone(),
                collected_at: collected_at.clone(),
                content_raw: item.content_raw.clone(),
            });
            if seen_fingerprints.contains(&normalized.fingerprint) {
                skipped_existing += 1;
                if self.ensure_xhs_review_queue_for_fingerprint(
                    conn,
                    &normalized.fingerprint,
                    &collected_at,
                )? {
                    backfilled_existing += 1;
                }
                continue;
            }

            let req = PipelinePreviewReq {
                source: "xhs".to_string(),
                source_url: Some(item.source_url.clone()),
                published_at: item.published_at.clone(),
                content_raw: item.content_raw.clone(),
            };
            match self.execute_pipeline_with_conn(conn, req) {
                Ok(exec_result) => {
                    let _ = persist_source_media_fields(
                        conn,
                        &exec_result.source_item_id,
                        item.cover_url.as_deref(),
                        &item.image_urls,
                    );
                    stored += 1;
                    seen_fingerprints.insert(normalized.fingerprint);
                }
                Err(e) => {
                    if !matches!(e, BackendError::Storage(_) | BackendError::Internal(_)) {
                        let payload = format!(
                            "{{\"external_id\":\"{}\",\"source_url\":\"{}\"}}",
                            super::escape_json(&item.external_id),
                            super::escape_json(&item.source_url)
                        );
                        let _ = self.record_dead_letter_raw(
                            conn,
                            "xhs_collect",
                            &item.external_id,
                            &payload,
                            e.code().as_str(),
                            &e.to_string(),
                        );
                    }
                }
            }
        }

        info!(
            source = "xhs",
            fetched, stored, skipped_existing, backfilled_existing, "collect requested"
        );
        Ok(CollectResult {
            source: "xhs".to_string(),
            fetched,
            stored,
        })
    }

    pub(super) fn apply_xhs_auto_pass_policy(
        &self,
        conn: &Connection,
        now: &str,
    ) -> Result<(u32, u32)> {
        let mut stmt = conn.prepare(
            "SELECT q.id,
                    q.analysis_result_id,
                    a.c_score,
                    a.value_score,
                    a.quality_score,
                    COALESCE(LENGTH(TRIM(s.content_raw)), 0) AS content_len
             FROM review_queue q
             JOIN normalized_items n ON n.id = q.normalized_item_id
             JOIN source_items s ON s.id = n.source_item_id
             JOIN analysis_results a ON a.id = q.analysis_result_id
             WHERE s.source = 'xhs'
               AND q.state = 'pending'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;

        let mut auto_done = 0u32;
        let mut kept_pending = 0u32;
        for row in rows {
            let (review_id, analysis_result_id, c_score, value_score, quality_score, content_len) =
                row?;
            let needs_manual = should_trigger_review(c_score, value_score)
                || quality_score < 65.0
                || content_len < XHS_MIN_CONTENT_CHARS;
            if needs_manual {
                kept_pending += 1;
                conn.execute(
                    "UPDATE review_queue
                     SET reason = ?2,
                         updated_at = ?3
                     WHERE id = ?1",
                    params![&review_id, XHS_REASON_MANUAL_REVIEW, now],
                )?;
                continue;
            }
            conn.execute(
                "UPDATE review_queue
                 SET state = 'done',
                     reason = ?2,
                     updated_at = ?3
                 WHERE id = ?1",
                params![&review_id, XHS_REASON_AUTO_PASS, now],
            )?;
            conn.execute(
                "UPDATE analysis_results
                 SET review_required = 0,
                     review_status = 'pass',
                     final_status = 'direct',
                     updated_at = ?2
                 WHERE id = ?1",
                params![&analysis_result_id, now],
            )?;
            auto_done += 1;
        }
        Ok((auto_done, kept_pending))
    }

    fn ensure_xhs_review_queue_for_fingerprint(
        &self,
        conn: &Connection,
        fingerprint: &str,
        now: &str,
    ) -> Result<bool> {
        let row: Option<(String, String, f64, f64, f64, i64)> = conn
            .query_row(
                "SELECT n.id,
                        a.id,
                        a.c_score,
                        a.value_score,
                        a.quality_score,
                        COALESCE(LENGTH(TRIM(s.content_raw)), 0) AS content_len
                 FROM normalized_items n
                 JOIN source_items s ON s.id = n.source_item_id
                 JOIN analysis_results a ON a.normalized_item_id = n.id
                 WHERE s.source = 'xhs' AND n.fingerprint = ?1
                 ORDER BY n.updated_at DESC
                 LIMIT 1",
                params![fingerprint],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            normalized_item_id,
            analysis_result_id,
            c_score,
            value_score,
            quality_score,
            content_len,
        )) = row
        else {
            return Ok(false);
        };

        let exists_any: i64 = conn.query_row(
            "SELECT COUNT(1) FROM review_queue WHERE analysis_result_id = ?1",
            params![&analysis_result_id],
            |r| r.get(0),
        )?;
        if exists_any > 0 {
            return Ok(false);
        }

        let review_id = format!(
            "rvw_bf_{:x}",
            super::stable_hash(&format!(
                "{normalized_item_id}|{analysis_result_id}|{fingerprint}"
            ))
        );
        let needs_manual = should_trigger_review(c_score, value_score)
            || quality_score < 65.0
            || content_len < XHS_MIN_CONTENT_CHARS;
        let (state, reason) = if needs_manual {
            ("pending", XHS_REASON_MANUAL_REVIEW)
        } else {
            ("done", XHS_REASON_AUTO_PASS)
        };
        conn.execute(
            "INSERT INTO review_queue(
                id, normalized_item_id, analysis_result_id, priority, reason, state, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                &review_id,
                &normalized_item_id,
                &analysis_result_id,
                0.60f64,
                reason,
                state,
                now
            ],
        )?;
        if !needs_manual {
            conn.execute(
                "UPDATE analysis_results
                 SET review_required = 0,
                     review_status = 'pass',
                     final_status = 'direct',
                     updated_at = ?2
                 WHERE id = ?1",
                params![&analysis_result_id, now],
            )?;
        }
        Ok(true)
    }

    fn load_known_xhs_fingerprints(&self, conn: &Connection) -> Result<HashSet<String>> {
        let mut stmt = conn.prepare(
            "SELECT n.fingerprint
             FROM normalized_items n
             JOIN source_items s ON s.id = n.source_item_id
             WHERE s.source = 'xhs'",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut set = HashSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    }
}

fn persist_source_media_fields(
    conn: &Connection,
    source_item_id: &str,
    cover_url: Option<&str>,
    image_urls: &[String],
) -> Result<()> {
    let normalized_cover = cover_url
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let mut deduped = Vec::new();
    for url in image_urls {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            continue;
        }
        if deduped.iter().any(|existing: &String| existing == trimmed) {
            continue;
        }
        deduped.push(trimmed.to_string());
    }
    if deduped.len() > 8 {
        deduped.truncate(8);
    }
    let image_urls_json = serde_json::to_string(&deduped)
        .map_err(|e| BackendError::Internal(format!("encode image_urls_json failed: {e}")))?;
    conn.execute(
        "UPDATE source_items
         SET cover_url=COALESCE(?2, cover_url),
             image_urls_json=?3
         WHERE id=?1",
        params![source_item_id, normalized_cover, image_urls_json],
    )?;
    Ok(())
}
