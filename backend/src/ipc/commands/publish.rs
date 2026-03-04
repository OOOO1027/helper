//! Publish pipeline domain: candidate selection, Notion sync dispatch, audit recording,
//! and publish queue / history queries.

use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::app_core::{AppCore, DateRange, PageReq, Paged};
use crate::{BackendError, Result};

use super::{
    now_ts, open_conn_from_env, page_bounds, read_app_config_i64, read_notion_sync_mode,
    run_notion_sync_once, sanitize_limit, PublishHistoryFilters, PublishHistoryItem,
    PublishResult, PublishTaskItem,
};

// ── Constants & private types ────────────────────────────────────────────────

pub(super) const PUBLISH_PIPELINE_STAGE: &str = "publish_approved_to_notion";

#[derive(Debug, Clone)]
struct PublishCandidate {
    sync_record_id: String,
    item_id: String,
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Seeds the `publish_tasks` table from `source_items`/`sync_records` if rows
/// are missing, and resets any stuck `processing` rows that are >15 min old.
pub(super) fn seed_publish_tasks_if_missing(conn: &rusqlite::Connection) -> Result<()> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO publish_tasks(
            id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
         )
         SELECT
            ('pt_' || s.id) AS id,
            s.id AS item_id,
            CASE
              WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'ignored'
              WHEN q.state = 'rejected' THEN 'ignored'
              WHEN q.state IN ('pending', 'processing') THEN 'ignored'
              WHEN COALESCE(sr.sync_state, 'pending') = 'success' THEN 'published'
              WHEN COALESCE(sr.sync_state, 'pending') IN ('failed', 'retry') THEN 'failed'
              ELSE 'pending'
            END AS state,
            COALESCE(sr.retry_count, 0) AS attempt_count,
            sr.next_retry_at,
            CASE
              WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source_inactive'
              WHEN q.state = 'rejected' THEN 'review_rejected'
              WHEN q.state IN ('pending', 'processing') THEN 'await_review'
              ELSE sr.last_error_code
            END AS error_code,
            CASE
              WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source marked inactive'
              WHEN q.state = 'rejected' THEN 'manual review rejected'
              WHEN q.state IN ('pending', 'processing') THEN 'await manual review'
              ELSE sr.last_error_message
            END AS error_message,
            COALESCE(sr.created_at, ?1),
            ?1
         FROM source_items s
         LEFT JOIN normalized_items n ON n.source_item_id = s.id
         LEFT JOIN review_queue q ON q.normalized_item_id = n.id
         LEFT JOIN sync_records sr ON sr.normalized_item_id = n.id AND sr.target = 'notion'
         LEFT JOIN source_state ss ON ss.item_id = s.id
         WHERE s.source = 'xhs'
           AND (q.state IS NOT NULL OR sr.id IS NOT NULL)
         ON CONFLICT(id) DO NOTHING",
        params![&now],
    )?;
    conn.execute(
        "UPDATE publish_tasks
         SET state='failed',
             attempt_count=attempt_count + 1,
             next_retry_at=NULL,
             error_code=CASE
               WHEN COALESCE(error_code, '') = '' THEN 'publish_stuck'
               ELSE error_code
             END,
             error_message=CASE
               WHEN COALESCE(error_message, '') = '' THEN 'processing timeout'
               ELSE error_message
             END,
             updated_at=?1
         WHERE state='processing'
           AND datetime(updated_at) <= datetime(?1, '-15 minutes')",
        params![&now],
    )?;
    conn.execute(
        "UPDATE publish_tasks
         SET state='pending',
             attempt_count=COALESCE(src.retry_count, 0),
             next_retry_at=src.next_retry_at,
             error_code=src.last_error_code,
             error_message=src.last_error_message,
             updated_at=?1
         FROM (
            SELECT
              sr.id AS sync_record_id,
              ('pt_' || origin.id) AS publish_task_id,
              sr.retry_count,
              sr.next_retry_at,
              sr.last_error_code,
              sr.last_error_message,
              origin.source AS source,
              q.state AS review_state,
              COALESCE(ss.active_state, 'active') AS active_state
            FROM sync_records sr
            JOIN normalized_items n ON n.id = sr.normalized_item_id
            JOIN source_items origin ON origin.id = n.source_item_id
            LEFT JOIN review_queue q ON q.normalized_item_id = n.id
            LEFT JOIN source_state ss ON ss.item_id = origin.id
            WHERE sr.target='notion'
              AND sr.sync_state IN ('pending','retry')
         ) AS src
         WHERE publish_tasks.id = src.publish_task_id
           AND (
             publish_tasks.state = 'published'
             OR (
               publish_tasks.state = 'ignored'
               AND COALESCE(publish_tasks.error_code, '') = ''
             )
           )
           AND src.active_state = 'active'
           AND COALESCE(src.review_state, '') != 'rejected'
           AND NOT (src.source = 'xhs' AND COALESCE(src.review_state, '') IN ('pending','processing'))",
        params![&now],
    )?;
    Ok(())
}

fn resolve_notion_sync_limit(
    conn: &rusqlite::Connection,
    requested: Option<usize>,
) -> Result<usize> {
    let cfg_limit = read_app_config_i64(conn, "notion.sync.default_limit")?.unwrap_or(50);
    let raw_limit = requested.unwrap_or_else(|| {
        if cfg_limit <= 0 {
            0
        } else {
            cfg_limit as usize
        }
    });
    Ok(sanitize_limit(raw_limit))
}

fn load_publish_candidates(
    conn: &rusqlite::Connection,
    limit: usize,
) -> Result<Vec<PublishCandidate>> {
    let now = now_ts();
    let mut stmt = conn.prepare(
        "SELECT
            sr.id,
            p.item_id
         FROM publish_tasks p
         JOIN normalized_items n ON n.source_item_id = p.item_id
         JOIN sync_records sr ON sr.normalized_item_id = n.id AND sr.target='notion'
         LEFT JOIN source_state ss ON ss.item_id = p.item_id
         WHERE p.state IN ('pending', 'failed')
           AND (p.next_retry_at IS NULL OR p.next_retry_at <= ?1)
           AND COALESCE(ss.active_state, 'active') = 'active'
         ORDER BY p.updated_at ASC
         LIMIT ?2",
    )?;
    let mut rows = stmt.query(params![now, limit as i64])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(PublishCandidate {
            sync_record_id: row.get(0)?,
            item_id: row.get(1)?,
        });
    }
    Ok(out)
}

fn mark_candidates_processing(
    conn: &rusqlite::Connection,
    candidates: &[PublishCandidate],
) -> Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let now = now_ts();
    for candidate in candidates {
        conn.execute(
            "UPDATE publish_tasks
             SET state='processing',
                 updated_at=?2
             WHERE id=?1
               AND state IN ('pending','failed')",
            params![format!("pt_{}", &candidate.item_id), &now],
        )?;
        conn.execute(
            "UPDATE sync_records
             SET sync_state='pending',
                 next_retry_at=NULL,
                 last_error_code=NULL,
                 last_error_message=NULL,
                 updated_at=?2
             WHERE id=?1",
            params![&candidate.sync_record_id, &now],
        )?;
    }
    Ok(())
}

fn mark_candidates_failed_from_batch_error(
    conn: &rusqlite::Connection,
    candidates: &[PublishCandidate],
    error_code: &str,
    error_message: &str,
) -> Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let now = now_ts();
    for candidate in candidates {
        conn.execute(
            "UPDATE publish_tasks
             SET state='failed',
                 attempt_count=attempt_count + 1,
                 next_retry_at=NULL,
                 error_code=?3,
                 error_message=?4,
                 updated_at=?2
             WHERE id=?1",
            params![
                format!("pt_{}", &candidate.item_id),
                &now,
                error_code,
                error_message
            ],
        )?;
        conn.execute(
            "UPDATE sync_records
             SET sync_state='failed',
                 retry_count=retry_count + 1,
                 next_retry_at=NULL,
                 last_error_code=?3,
                 last_error_message=?4,
                 updated_at=?2
             WHERE id=?1",
            params![&candidate.sync_record_id, &now, error_code, error_message],
        )?;
    }
    Ok(())
}

fn upsert_notion_page_refs_for_candidates(
    conn: &rusqlite::Connection,
    candidates: &[PublishCandidate],
) -> Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let now = now_ts();
    for candidate in candidates {
        let row = conn
            .query_row(
                "SELECT
                    sr.target_record_id,
                    COALESCE(t.category_name, 'Inbox'),
                    t.week_key,
                    t.route_reason,
                    n.fingerprint
                 FROM sync_records sr
                 JOIN normalized_items n ON n.id = sr.normalized_item_id
                 LEFT JOIN notion_tree_nodes t ON t.node_type='item' AND t.normalized_item_id = sr.normalized_item_id
                 WHERE sr.id = ?1
                   AND sr.sync_state = 'success'
                   AND sr.target_record_id IS NOT NULL",
                params![&candidate.sync_record_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((notion_page_id, category, week_key, route_reason, published_hash)) = row else {
            continue;
        };
        conn.execute(
            "INSERT INTO notion_page_refs(
                id, item_id, notion_page_id, category, week_key, route_reason, published_hash, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(item_id) DO UPDATE SET
                notion_page_id=excluded.notion_page_id,
                category=excluded.category,
                week_key=excluded.week_key,
                route_reason=excluded.route_reason,
                published_hash=excluded.published_hash,
                updated_at=excluded.updated_at",
            params![
                format!("nref_{}", &candidate.item_id),
                &candidate.item_id,
                notion_page_id,
                category,
                week_key,
                route_reason,
                published_hash,
                &now
            ],
        )?;
    }
    Ok(())
}

fn insert_publish_audit_row(
    conn: &rusqlite::Connection,
    request_id: &str,
    publish_task_id: Option<&str>,
    item_id: &str,
    status: &str,
    latency_ms: i64,
    error_code: Option<&str>,
    error_message: Option<&str>,
    pipeline_stage: &str,
    sync_mode: &str,
    retryable: bool,
) -> Result<()> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO publish_audit(
            id, publish_task_id, item_id, request_id, status, latency_ms, error_code, error_message,
            pipeline_stage, sync_mode, retryable, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            format!("pa_{}", Uuid::new_v4()),
            publish_task_id,
            item_id,
            request_id,
            status,
            latency_ms,
            error_code,
            error_message,
            pipeline_stage,
            sync_mode,
            if retryable { 1 } else { 0 },
            now
        ],
    )?;
    Ok(())
}

fn is_retryable_publish_error_code(error_code: Option<&str>) -> bool {
    matches!(
        error_code,
        Some("MOD-3002" | "MOD-3429" | "DB-4001" | "INT-9000")
    )
}

fn record_publish_audit_for_candidates(
    conn: &rusqlite::Connection,
    request_id: &str,
    candidates: &[PublishCandidate],
    latency_ms: i64,
    pipeline_stage: &str,
    sync_mode: &str,
    fallback_error: Option<(&str, &str, bool)>,
) -> Result<()> {
    if candidates.is_empty() {
        let (error_code, error_message, retryable) =
            if let Some((code, message, retryable)) = fallback_error {
                (Some(code), Some(message), retryable)
            } else {
                (None, None, false)
            };
        return insert_publish_audit_row(
            conn,
            request_id,
            None,
            "__batch__",
            if error_code.is_some() {
                "failed"
            } else {
                "skipped"
            },
            latency_ms.max(0),
            error_code,
            error_message,
            pipeline_stage,
            sync_mode,
            retryable,
        );
    }

    let per_item_latency = (latency_ms / candidates.len() as i64).max(0);
    for candidate in candidates {
        let mut status = "skipped".to_string();
        let mut error_code: Option<String> = None;
        let mut error_message: Option<String> = None;
        let mut retryable = false;
        if let Some((code, message, candidate_retryable)) = fallback_error {
            status = "failed".to_string();
            error_code = Some(code.to_string());
            error_message = Some(message.to_string());
            retryable = candidate_retryable;
        } else {
            let row = conn
                .query_row(
                    "SELECT sync_state, last_error_code, last_error_message
                     FROM sync_records
                     WHERE id = ?1",
                    params![&candidate.sync_record_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((sync_state, code, message)) = row {
                status = match sync_state.as_str() {
                    "success" => "success".to_string(),
                    "retry" => "partial".to_string(),
                    "failed" => "failed".to_string(),
                    _ => "skipped".to_string(),
                };
                error_code = code;
                error_message = message;
                retryable =
                    sync_state == "retry" || is_retryable_publish_error_code(error_code.as_deref());
            }
        }
        insert_publish_audit_row(
            conn,
            request_id,
            Some(&format!("pt_{}", &candidate.item_id)),
            &candidate.item_id,
            &status,
            per_item_latency,
            error_code.as_deref(),
            error_message.as_deref(),
            pipeline_stage,
            sync_mode,
            retryable,
        )?;
    }
    Ok(())
}

// ── Public API ────────────────────────────────────────────────────────────────

pub async fn publish_approved_to_notion(
    app: &AppCore,
    limit: Option<usize>,
) -> Result<PublishResult> {
    let conn = open_conn_from_env()?;
    seed_publish_tasks_if_missing(&conn)?;
    let sync_mode = read_notion_sync_mode(Some(&conn))?;
    let effective_limit = resolve_notion_sync_limit(&conn, limit)?;
    let candidates = load_publish_candidates(&conn, effective_limit)?;
    let request_id = format!("pub_{}", Uuid::new_v4());
    if candidates.is_empty() {
        let _ = record_publish_audit_for_candidates(
            &conn,
            &request_id,
            &candidates,
            0,
            PUBLISH_PIPELINE_STAGE,
            &sync_mode,
            None,
        );
        return Ok(PublishResult {
            request_id,
            attempted: 0,
            succeeded: 0,
            failed: 0,
            requeued: 0,
            dead_lettered: 0,
        });
    }
    mark_candidates_processing(&conn, &candidates)?;
    let started = std::time::Instant::now();
    match run_notion_sync_once(app, Some(effective_limit)).await {
        Ok(summary) => {
            let latency_ms = started.elapsed().as_millis() as i64;
            let _ = upsert_notion_page_refs_for_candidates(&conn, &candidates);
            let _ = record_publish_audit_for_candidates(
                &conn,
                &request_id,
                &candidates,
                latency_ms,
                PUBLISH_PIPELINE_STAGE,
                &sync_mode,
                None,
            );
            let _ = seed_publish_tasks_if_missing(&conn);
            Ok(PublishResult {
                request_id,
                attempted: summary.attempted as u32,
                succeeded: summary.succeeded as u32,
                failed: summary.failed as u32,
                requeued: summary.requeued as u32,
                dead_lettered: summary.dead_lettered as u32,
            })
        }
        Err(err) => {
            let latency_ms = started.elapsed().as_millis() as i64;
            let err_text = err.to_string();
            let _ = mark_candidates_failed_from_batch_error(
                &conn,
                &candidates,
                err.code().as_str(),
                &err_text,
            );
            let _ = record_publish_audit_for_candidates(
                &conn,
                &request_id,
                &candidates,
                latency_ms,
                PUBLISH_PIPELINE_STAGE,
                &sync_mode,
                Some((err.code().as_str(), &err_text, err.retryable())),
            );
            Err(err)
        }
    }
}

pub fn get_publish_queue(_app: &AppCore, page: PageReq) -> Result<Paged<PublishTaskItem>> {
    let conn = open_conn_from_env()?;
    seed_publish_tasks_if_missing(&conn)?;
    let (page_size, offset) = page_bounds(&page)?;

    let total: i64 = conn.query_row("SELECT COUNT(1) FROM publish_tasks", [], |r| r.get(0))?;
    let mut stmt = conn.prepare(
        "SELECT
            p.id,
            p.item_id,
            COALESCE(NULLIF(TRIM(s.title), ''), NULLIF(TRIM(substr(s.content_raw, 1, 120)), ''), '未命名内容') AS title,
            p.state,
            p.attempt_count,
            p.next_retry_at,
            p.error_code,
            p.updated_at
         FROM publish_tasks p
         LEFT JOIN source_items s ON s.id = p.item_id
         ORDER BY p.updated_at DESC
         LIMIT ?1 OFFSET ?2",
    )?;
    let mut rows = stmt.query(params![page_size, offset])?;
    let mut items = Vec::new();
    while let Some(row) = rows.next()? {
        items.push(PublishTaskItem {
            id: row.get(0)?,
            item_id: row.get(1)?,
            title: row.get(2)?,
            state: row.get(3)?,
            attempt_count: row.get::<_, i64>(4)?.max(0) as u32,
            next_retry_at: row.get(5)?,
            error_code: row.get(6)?,
            updated_at: row.get(7)?,
        });
    }

    Ok(Paged {
        items,
        page: page.page,
        page_size: page.page_size,
        total: total.max(0) as u64,
    })
}

pub fn get_publish_history(
    _app: &AppCore,
    range: DateRange,
    page: PageReq,
) -> Result<Paged<PublishHistoryItem>> {
    get_publish_history_with_filters(_app, range, page, PublishHistoryFilters::default())
}

pub fn get_publish_history_with_filters(
    _app: &AppCore,
    range: DateRange,
    page: PageReq,
    filters: PublishHistoryFilters,
) -> Result<Paged<PublishHistoryItem>> {
    if range.from.trim().is_empty() || range.to.trim().is_empty() {
        return Err(BackendError::Validation(
            "range.from and range.to must not be empty".to_string(),
        ));
    }
    let conn = open_conn_from_env()?;
    let (page_size, offset) = page_bounds(&page)?;
    let state_filter = normalize_history_string_filter(filters.state);
    let sync_mode_filter = normalize_history_string_filter(filters.sync_mode);
    let retryable_filter = filters.retryable.map(|v| if v { 1_i64 } else { 0_i64 });

    let audit_group_total: i64 = conn.query_row(
        "SELECT COUNT(1)
         FROM (
           SELECT request_id
           FROM publish_audit
           WHERE created_at >= ?1
             AND created_at <= ?2
           GROUP BY request_id
         ) t",
        params![range.from, range.to],
        |r| r.get(0),
    )?;
    if audit_group_total > 0 {
        let total: i64 = conn.query_row(
            "WITH grouped AS (
               SELECT
                   request_id,
                   CASE
                     WHEN SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END) > 0
                          AND SUM(CASE WHEN status='success' THEN 1 ELSE 0 END) > 0 THEN 'partial'
                     WHEN SUM(CASE WHEN status='failed' THEN 1 ELSE 0 END) > 0 THEN 'failed'
                     WHEN SUM(CASE WHEN status='partial' THEN 1 ELSE 0 END) > 0 THEN 'partial'
                     WHEN SUM(CASE WHEN status='success' THEN 1 ELSE 0 END) > 0 THEN 'success'
                     ELSE 'skipped'
                   END AS state,
                   MAX(sync_mode) AS sync_mode,
                   MAX(retryable) AS retryable,
                   MAX(created_at) AS created_at
                FROM publish_audit
                WHERE created_at >= ?1
                  AND created_at <= ?2
                GROUP BY request_id
             )
             SELECT COUNT(1)
             FROM grouped
             WHERE (?3 IS NULL OR state = ?3)
               AND (?4 IS NULL OR sync_mode = ?4)
               AND (?5 IS NULL OR retryable = ?5)",
            params![
                range.from,
                range.to,
                state_filter,
                sync_mode_filter,
                retryable_filter
            ],
            |r| r.get(0),
        )?;
        if total == 0 {
            return Ok(Paged {
                items: Vec::new(),
                page: page.page,
                page_size: page.page_size,
                total: 0,
            });
        }
        let mut stmt = conn.prepare(
            "WITH grouped AS (
               SELECT
                   request_id,
                   CASE
                     WHEN SUM(CASE WHEN pa.status='failed' THEN 1 ELSE 0 END) > 0
                          AND SUM(CASE WHEN pa.status='success' THEN 1 ELSE 0 END) > 0 THEN 'partial'
                     WHEN SUM(CASE WHEN pa.status='failed' THEN 1 ELSE 0 END) > 0 THEN 'failed'
                     WHEN SUM(CASE WHEN pa.status='partial' THEN 1 ELSE 0 END) > 0 THEN 'partial'
                     WHEN SUM(CASE WHEN pa.status='success' THEN 1 ELSE 0 END) > 0 THEN 'success'
                     ELSE 'skipped'
                   END AS state,
                   MAX(pa.error_code) AS error_code,
                   MAX(pa.created_at) AS created_at,
                   SUM(CASE WHEN pa.status='success' THEN 1 ELSE 0 END) AS success_count,
                   SUM(CASE WHEN pa.status IN ('failed','partial') THEN 1 ELSE 0 END) AS fail_count,
                   MAX(pa.pipeline_stage) AS pipeline_stage,
                   MAX(pa.sync_mode) AS sync_mode,
                   MAX(pa.retryable) AS retryable,
                   MAX(npr.category) AS category,
                   MAX(npr.week_key) AS week_key,
                   MAX(npr.route_reason) AS route_reason,
                   MAX(ar.summary_text) AS conclusion_summary,
                   MAX(ar.classification_json) AS classification_json,
                   MAX(si.cover_url) AS cover_url,
                   MAX(si.image_urls_json) AS image_urls_json
                FROM publish_audit pa
                LEFT JOIN notion_page_refs npr ON npr.item_id = pa.item_id
                LEFT JOIN source_items si ON si.id = pa.item_id
                LEFT JOIN normalized_items ni ON ni.source_item_id = pa.item_id
                LEFT JOIN analysis_results ar ON ar.normalized_item_id = ni.id
                WHERE pa.created_at >= ?1
                  AND pa.created_at <= ?2
                GROUP BY pa.request_id
             )
             SELECT
                 request_id,
                 state,
                 error_code,
                 created_at,
                 success_count,
                 fail_count,
                 pipeline_stage,
                 sync_mode,
                 retryable,
                 category,
                 week_key,
                 route_reason,
                 conclusion_summary,
                 classification_json,
                 cover_url,
                 image_urls_json
             FROM grouped
             WHERE (?3 IS NULL OR state = ?3)
               AND (?4 IS NULL OR sync_mode = ?4)
               AND (?5 IS NULL OR retryable = ?5)
             ORDER BY created_at DESC
             LIMIT ?6 OFFSET ?7",
        )?;
        let mut rows = stmt.query(params![
            range.from,
            range.to,
            state_filter,
            sync_mode_filter,
            retryable_filter,
            page_size,
            offset
        ])?;
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            let classification_json: Option<String> = row.get(13)?;
            let parsed_classification =
                parse_publish_classification_payload(classification_json.as_deref());
            items.push(PublishHistoryItem {
                id: row.get(0)?,
                state: row.get(1)?,
                error_code: row.get(2)?,
                created_at: row.get(3)?,
                success_count: row.get::<_, i64>(4)?.max(0) as u32,
                fail_count: row.get::<_, i64>(5)?.max(0) as u32,
                pipeline_stage: row.get(6)?,
                sync_mode: row.get(7)?,
                retryable: row.get::<_, Option<i64>>(8)?.map(|v| v != 0),
                category: row.get(9)?,
                week_key: row.get(10)?,
                route_reason: row.get(11)?,
                conclusion_summary: row.get(12)?,
                key_points: parsed_classification.key_points,
                tags: parsed_classification.tags,
                cover_url: row.get(14)?,
                image_urls: parse_json_string_list(row.get::<_, Option<String>>(15)?),
                quality_state: parsed_classification.quality_state,
                quality_score: parsed_classification.quality_score,
                degraded_fields: parsed_classification.degraded_fields,
            });
        }
        return Ok(Paged {
            items,
            page: page.page,
            page_size: page.page_size,
            total: total.max(0) as u64,
        });
    }

    // Compatibility fallback for older data before publish_audit rollout.
    // Legacy job_runs has no sync_mode/retryable fields; if either filter is provided,
    // return empty instead of guessing semantics.
    if sync_mode_filter.is_some() || retryable_filter.is_some() {
        return Ok(Paged {
            items: Vec::new(),
            page: page.page,
            page_size: page.page_size,
            total: 0,
        });
    }

    let legacy_total: i64 = conn.query_row(
        "SELECT COUNT(1)
         FROM job_runs
         WHERE job_type = 'notion_sync_once'
           AND created_at >= ?1
           AND created_at <= ?2
           AND (?3 IS NULL OR status = ?3)",
        params![range.from, range.to, state_filter],
        |r| r.get(0),
    )?;
    let mut legacy_stmt = conn.prepare(
        "SELECT id, status, error_code, created_at, success_count, fail_count
         FROM job_runs
         WHERE job_type = 'notion_sync_once'
           AND created_at >= ?1
           AND created_at <= ?2
           AND (?3 IS NULL OR status = ?3)
         ORDER BY created_at DESC
         LIMIT ?4 OFFSET ?5",
    )?;
    let mut legacy_rows = legacy_stmt.query(params![
        range.from,
        range.to,
        state_filter,
        page_size,
        offset
    ])?;
    let mut legacy_items = Vec::new();
    while let Some(row) = legacy_rows.next()? {
        legacy_items.push(PublishHistoryItem {
            id: row.get(0)?,
            state: row.get(1)?,
            error_code: row.get(2)?,
            created_at: row.get(3)?,
            success_count: row.get::<_, i64>(4)?.max(0) as u32,
            fail_count: row.get::<_, i64>(5)?.max(0) as u32,
            pipeline_stage: None,
            sync_mode: None,
            retryable: None,
            category: None,
            week_key: None,
            route_reason: None,
            conclusion_summary: None,
            key_points: None,
            tags: None,
            cover_url: None,
            image_urls: None,
            quality_state: None,
            quality_score: None,
            degraded_fields: None,
        });
    }

    Ok(Paged {
        items: legacy_items,
        page: page.page,
        page_size: page.page_size,
        total: legacy_total.max(0) as u64,
    })
}

// ── History query helpers ─────────────────────────────────────────────────────

fn normalize_history_string_filter(raw: Option<String>) -> Option<String> {
    raw.and_then(|v| {
        let normalized = v.trim().to_string();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

#[derive(Debug, Clone, Default)]
struct PublishClassificationPayload {
    key_points: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    quality_state: Option<String>,
    quality_score: Option<f64>,
    degraded_fields: Option<Vec<String>>,
}

fn parse_publish_classification_payload(raw: Option<&str>) -> PublishClassificationPayload {
    let mut out = PublishClassificationPayload::default();
    let Some(value) = raw.map(|v| v.trim()).filter(|v| !v.is_empty()) else {
        return out;
    };
    let parsed = match serde_json::from_str::<serde_json::Value>(value) {
        Ok(v) => v,
        Err(_) => return out,
    };
    if let Some(map) = parsed.as_object() {
        out.key_points = extract_json_list(map.get("key_points"));
        out.tags = extract_json_list(map.get("tags"));
        out.quality_state = map
            .get("quality_state")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        out.quality_score = map.get("quality_score").and_then(|v| v.as_f64());
        out.degraded_fields = extract_json_list(map.get("degraded_fields"));
    } else if parsed.is_array() {
        out.tags = extract_json_list(Some(&parsed));
    }
    out
}

fn extract_json_list(value: Option<&serde_json::Value>) -> Option<Vec<String>> {
    let mut out = Vec::<String>::new();
    if let Some(serde_json::Value::Array(items)) = value {
        for item in items {
            if let Some(text) = item.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_json_string_list(raw: Option<String>) -> Option<Vec<String>> {
    let value = raw?.trim().to_string();
    if value.is_empty() {
        return None;
    }
    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&value) {
        let list = parsed
            .into_iter()
            .map(|entry| entry.trim().to_string())
            .filter(|entry| !entry.is_empty())
            .collect::<Vec<_>>();
        if list.is_empty() {
            None
        } else {
            Some(list)
        }
    } else {
        None
    }
}
