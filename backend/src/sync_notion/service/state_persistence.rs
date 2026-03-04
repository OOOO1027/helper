use chrono::{Duration, Local};
use rusqlite::{params, Connection};
use serde_json::json;

use super::{ModelUsageRecord, PendingSyncRow, StructuredContent};
use crate::{BackendError, Result};

pub(super) fn persist_structured_snapshot(
    conn: &Connection,
    row: &PendingSyncRow,
    content: &StructuredContent,
    category: &str,
    route_reason: Option<&str>,
) -> Result<()> {
    let payload = json!({
        "labels": row.labels,
        "tags": content.tags,
        "key_points": content.key_points,
        "final_category": category,
        "route_reason": route_reason,
        "quality_state": content.quality_state,
        "quality_score": content.quality_score,
        "degraded_fields": content.degraded_fields,
        "content_source": content.content_source,
    });
    let payload_json = serde_json::to_string(&payload)
        .map_err(|e| BackendError::Internal(format!("classification json encode failed: {e}")))?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE analysis_results
         SET summary_text=?2,
             classification_json=?3,
             updated_at=?4
         WHERE normalized_item_id=?1",
        params![
            &row.normalized_item_id,
            &content.summary,
            payload_json,
            &now
        ],
    )?;
    if let Some(usage) = content.usage.as_ref() {
        insert_budget_ledger_usage(conn, usage, &now)?;
    }
    Ok(())
}

fn insert_budget_ledger_usage(
    conn: &Connection,
    usage: &ModelUsageRecord,
    created_at: &str,
) -> Result<()> {
    let day = created_at.get(0..10).unwrap_or(created_at).to_string();
    conn.execute(
        "INSERT INTO budget_ledger(
            id, day, provider, model, purpose, tokens_in, tokens_out, cost_cny, job_run_id, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9)",
        params![
            format!("bl_{}", uuid::Uuid::new_v4()),
            day,
            &usage.provider,
            &usage.model,
            &usage.purpose,
            usage.tokens_in as i64,
            usage.tokens_out as i64,
            usage.cost_cny,
            created_at
        ],
    )?;
    Ok(())
}

fn upsert_publish_task_state_by_sync_record(
    conn: &Connection,
    sync_record_id: &str,
    state: &str,
    attempt_count: u32,
    next_retry_at: Option<&str>,
    error_code: Option<&str>,
    error_message: Option<&str>,
    updated_at: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO publish_tasks(
            id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
         )
         SELECT
            ('pt_' || n.source_item_id) AS id,
            n.source_item_id AS item_id,
            ?2 AS state,
            ?3 AS attempt_count,
            ?4 AS next_retry_at,
            ?5 AS error_code,
            ?6 AS error_message,
            ?7 AS created_at,
            ?7 AS updated_at
         FROM sync_records s
         JOIN normalized_items n ON n.id = s.normalized_item_id
         WHERE s.id = ?1
         ON CONFLICT(id) DO UPDATE SET
            state=excluded.state,
            attempt_count=excluded.attempt_count,
            next_retry_at=excluded.next_retry_at,
            error_code=excluded.error_code,
            error_message=excluded.error_message,
            updated_at=excluded.updated_at",
        params![
            sync_record_id,
            state,
            attempt_count as i64,
            next_retry_at,
            error_code,
            error_message,
            updated_at
        ],
    )?;
    Ok(())
}

pub(super) fn mark_sync_success(
    conn: &Connection,
    sync_record_id: &str,
    target_record_id: Option<&str>,
) -> Result<()> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE sync_records
         SET sync_state='success',
             retry_count=0,
             next_retry_at=NULL,
             last_error_code=NULL,
             last_error_message=NULL,
             target_record_id=COALESCE(?3, target_record_id),
             last_synced_at=?2,
             updated_at=?2
         WHERE id=?1",
        params![sync_record_id, now, target_record_id],
    )?;
    upsert_publish_task_state_by_sync_record(
        conn,
        sync_record_id,
        "published",
        0,
        None,
        None,
        None,
        &now,
    )?;
    Ok(())
}

pub(super) fn mark_sync_retry(
    conn: &Connection,
    sync_record_id: &str,
    next_retry_count: u32,
    backoff_ms: u64,
    err: &BackendError,
    sync_error_code: &str,
) -> Result<()> {
    let now = Local::now();
    let retry_at = now + Duration::milliseconds(backoff_ms as i64);
    let retry_at_str = retry_at.format("%Y-%m-%d %H:%M:%S").to_string();
    let now_str = now.format("%Y-%m-%d %H:%M:%S").to_string();
    let err_text = err.to_string();
    conn.execute(
        "UPDATE sync_records
         SET sync_state='retry',
             retry_count=?2,
             next_retry_at=?3,
             last_error_code=?4,
             last_error_message=?5,
             updated_at=?6
         WHERE id=?1",
        params![
            sync_record_id,
            next_retry_count as i64,
            &retry_at_str,
            sync_error_code,
            &err_text,
            &now_str
        ],
    )?;
    upsert_publish_task_state_by_sync_record(
        conn,
        sync_record_id,
        "failed",
        next_retry_count,
        Some(&retry_at_str),
        Some(sync_error_code),
        Some(&err_text),
        &now_str,
    )?;
    Ok(())
}

pub(super) fn mark_sync_failed(
    conn: &Connection,
    sync_record_id: &str,
    next_retry_count: u32,
    err: &BackendError,
    sync_error_code: &str,
) -> Result<()> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let err_text = err.to_string();
    conn.execute(
        "UPDATE sync_records
         SET sync_state='failed',
             retry_count=?2,
             next_retry_at=NULL,
             last_error_code=?3,
             last_error_message=?4,
             updated_at=?5
         WHERE id=?1",
        params![
            sync_record_id,
            next_retry_count as i64,
            sync_error_code,
            &err_text,
            &now
        ],
    )?;
    upsert_publish_task_state_by_sync_record(
        conn,
        sync_record_id,
        "failed",
        next_retry_count,
        None,
        Some(sync_error_code),
        Some(&err_text),
        &now,
    )?;
    Ok(())
}

pub(super) fn classify_sync_error_code(err: &BackendError) -> &'static str {
    let msg = err.to_string().to_ascii_lowercase();
    if msg.contains("429")
        || msg.contains("rate limit")
        || msg.contains("rate_limited")
        || msg.contains("too many requests")
    {
        "MOD-3429"
    } else {
        "MOD-3002"
    }
}

pub(super) fn is_quality_gate_error(err: &BackendError) -> bool {
    err.to_string()
        .to_ascii_lowercase()
        .contains("content quality gate failed")
}

pub(super) fn insert_dead_letter(
    conn: &Connection,
    entity_id: &str,
    stage: &str,
    error_code: &str,
    error_message: &str,
) -> Result<()> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let id = format!(
        "dlq_sync_{:x}",
        super::stable_hash(&format!("{entity_id}|{stage}|{error_code}|{error_message}"))
    );
    conn.execute(
        "INSERT INTO dead_letter(
            id, entity_type, entity_id, stage, payload_json, error_code, error_message, attempt_count, state, created_at
         ) VALUES (?1, 'sync_record', ?2, ?3, '{}', ?4, ?5, 3, 'open', ?6)",
        params![id, entity_id, stage, error_code, error_message, now],
    )?;
    Ok(())
}
