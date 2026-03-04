use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json;
use uuid::Uuid;

use crate::pipeline::analyze::{analyze, AnalyzeInput, ContentAnalysisInput};
use crate::pipeline::normalize::{normalize, NormalizeInput};
use crate::pipeline::score::{gate_bucket, should_trigger_review, GateBucket};
use crate::sync_notion::client::{build_upsert_plan, NotionRecord, UpsertAction};
use crate::{BackendError, Result};

use super::{AppCore, PipelineExecuteResult, PipelinePreviewReq, PipelinePreviewResult};

impl AppCore {
    pub fn gate_decision(&self, c: f64, s: f64, j: f64, d: f64, value: f64) -> GateBucket {
        let quality = crate::pipeline::score::quality_score(c, s, j, d);
        let requires_review = should_trigger_review(c, value) && self.budget.review_enabled();
        if requires_review {
            GateBucket::Review
        } else {
            gate_bucket(quality)
        }
    }

    pub fn preview_pipeline(&self, req: PipelinePreviewReq) -> Result<PipelinePreviewResult> {
        if req.content_raw.trim().is_empty() {
            return Err(BackendError::Validation(
                "content_raw must not be empty".to_string(),
            ));
        }
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let normalized = normalize(&NormalizeInput {
            source: req.source.clone(),
            source_url: req.source_url.clone().unwrap_or_default(),
            published_at: req.published_at.clone(),
            collected_at: now,
            content_raw: req.content_raw.clone(),
        });

        let metrics = AnalyzeInput {
            c: 0.82,
            s: 0.79,
            j: 0.80,
            d: 0.75,
            r: 0.70,
            f: 0.75,
            e: 0.80,
            p: 0.78,
        };
        let analyzed = analyze(&ContentAnalysisInput {
            metrics: metrics.clone(),
            text: normalized.text_clean.clone(),
        });
        let review_required = if normalized.source == "xhs" {
            super::xhs_collection::xhs_requires_manual_review(&analyzed, &normalized.text_clean)
        } else {
            analyzed.requires_review
        };
        let record = NotionRecord {
            normalized_item_id: Uuid::new_v4().to_string(),
            title: normalized.text_clean.chars().take(32).collect::<String>(),
            summary: analyzed.summary.clone(),
            labels: analyzed.labels.clone(),
            source: normalized.source.clone(),
            source_url: Some(normalized.canonical_url.clone()),
            published_at: normalized.published_at.clone(),
            confidence: analyzed.classification_confidence,
            quality_score: analyzed.quality_score,
            value_score: analyzed.value_score,
        };
        let plan = build_upsert_plan(&[record], &HashMap::new());
        let notion_action = plan
            .first()
            .map(|v| match v.action {
                UpsertAction::Create => "create",
                UpsertAction::Update => "update",
                UpsertAction::Skip => "skip",
            })
            .unwrap_or("skip")
            .to_string();

        let gate = match analyzed.gate {
            crate::pipeline::analyze::GateBucketDto::Direct => "direct",
            crate::pipeline::analyze::GateBucketDto::Review => "review",
            crate::pipeline::analyze::GateBucketDto::Exception => "exception",
        }
        .to_string();

        Ok(PipelinePreviewResult {
            dedupe_key: normalized.dedupe_key,
            labels: analyzed.labels,
            summary: analyzed.summary,
            classification_confidence: analyzed.classification_confidence,
            summary_confidence: analyzed.summary_confidence,
            review_required,
            gate,
            notion_action,
        })
    }

    pub fn execute_pipeline_with_conn(
        &self,
        conn: &mut Connection,
        req: PipelinePreviewReq,
    ) -> Result<PipelineExecuteResult> {
        let req_snapshot = req.clone();
        let exec = self.execute_pipeline_inner(conn, req);
        if let Err(err) = exec.as_ref() {
            if matches!(err, BackendError::Storage(_) | BackendError::Internal(_)) {
                let payload_json =
                    serde_json::to_string(&req_snapshot).unwrap_or_else(|_| "{}".to_string());
                let _ = self.record_dead_letter_with_payload(
                    conn,
                    "pipeline_execute",
                    "unknown",
                    &payload_json,
                    err,
                );
            }
        }
        exec
    }

    fn execute_pipeline_inner(
        &self,
        conn: &mut Connection,
        req: PipelinePreviewReq,
    ) -> Result<PipelineExecuteResult> {
        if req.content_raw.trim().is_empty() {
            return Err(BackendError::Validation(
                "content_raw must not be empty".to_string(),
            ));
        }

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let normalized = normalize(&NormalizeInput {
            source: req.source.clone(),
            source_url: req.source_url.clone().unwrap_or_default(),
            published_at: req.published_at.clone(),
            collected_at: now.clone(),
            content_raw: req.content_raw.clone(),
        });
        let source = normalized.source.clone();
        let metrics = AnalyzeInput {
            c: 0.82,
            s: 0.79,
            j: 0.80,
            d: 0.75,
            r: 0.70,
            f: 0.75,
            e: 0.80,
            p: 0.78,
        };
        let analyzed = analyze(&ContentAnalysisInput {
            metrics: metrics.clone(),
            text: normalized.text_clean.clone(),
        });
        let xhs_policy_review_required = source == "xhs"
            && super::xhs_collection::xhs_requires_manual_review(&analyzed, &normalized.text_clean);
        let review_required = if source == "xhs" {
            xhs_policy_review_required
        } else {
            analyzed.requires_review
        };
        let force_review = source == "xhs" && xhs_policy_review_required;
        let review_reason = if source == "xhs" {
            "XHS anomaly requires manual review"
        } else {
            "C < 0.78 OR ValueScore >= 0.85"
        };
        let canonical_url = normalized.canonical_url.clone();
        let url_hash = normalized.url_hash.clone();
        let text_clean = normalized.text_clean.clone();
        let published_at = normalized.published_at.clone();
        let collected_at = normalized.collected_at.clone();
        let fingerprint = normalized.fingerprint.clone();
        let dedupe_key = normalized.dedupe_key.clone();
        let gate_status = match analyzed.gate {
            crate::pipeline::analyze::GateBucketDto::Direct => "direct",
            crate::pipeline::analyze::GateBucketDto::Review => "review",
            crate::pipeline::analyze::GateBucketDto::Exception => "exception",
        };
        let final_status = if review_required {
            "review"
        } else if source == "xhs" {
            "direct"
        } else {
            gate_status
        };
        let external_id = format!(
            "ext_{:x}",
            super::stable_hash(&format!("{}|{}", url_hash, fingerprint))
        );
        let source_item_id = format!(
            "src_{:x}",
            super::stable_hash(&format!("{}|{}", source, external_id))
        );
        let normalized_item_id = format!("norm_{}", dedupe_key);
        let analysis_result_id = format!("ana_{}", dedupe_key);
        let sync_record_id = format!("sync_{}", dedupe_key);

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO source_items(
                id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'new')
             ON CONFLICT(source, external_id) DO UPDATE SET
                source_url=excluded.source_url,
                url_hash=excluded.url_hash,
                title=excluded.title,
                content_raw=excluded.content_raw,
                published_at=excluded.published_at,
                collected_at=excluded.collected_at",
            params![
                &source_item_id,
                &source,
                &external_id,
                &canonical_url,
                &url_hash,
                text_clean.chars().take(64).collect::<String>(),
                &text_clean,
                published_at.as_deref(),
                &collected_at
            ],
        )?;

        tx.execute(
            "INSERT INTO normalized_items(
                id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint,
                quality_score, value_score, gate_status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
             ON CONFLICT(source_item_id) DO UPDATE SET
                canonical_url=excluded.canonical_url,
                canonical_url_hash=excluded.canonical_url_hash,
                text_clean=excluded.text_clean,
                fingerprint=excluded.fingerprint,
                quality_score=excluded.quality_score,
                value_score=excluded.value_score,
                gate_status=excluded.gate_status,
                updated_at=excluded.updated_at",
            params![
                &normalized_item_id,
                &source_item_id,
                &canonical_url,
                &url_hash,
                &text_clean,
                &fingerprint,
                analyzed.quality_score,
                analyzed.value_score,
                gate_status,
                &now
            ],
        )?;

        tx.execute(
            "INSERT INTO dedupe_index(
                id, normalized_item_id, url_hash, simhash, embedding_ref, rule_type,
                confidence, duplicate_of, first_seen_at, last_seen_at
             ) VALUES (?1, ?2, ?3, NULL, NULL, 'hard_url', 1.0, NULL, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET
                last_seen_at=excluded.last_seen_at",
            params![
                format!("dup_{}", dedupe_key),
                &normalized_item_id,
                &url_hash,
                &now
            ],
        )?;

        let classification_payload = serde_json::json!({
            "labels": analyzed.labels.clone(),
            "tags": analyzed.tags.clone(),
            "key_points": analyzed.key_points.clone(),
            "final_category": analyzed.final_category.clone(),
            "route_reason": analyzed.route_reason.clone(),
        });
        let classification_json = serde_json::to_string(&classification_payload).map_err(|e| {
            BackendError::Internal(format!("classification json encode failed: {e}"))
        })?;
        tx.execute(
            "INSERT INTO analysis_results(
                id, normalized_item_id, primary_model, review_model,
                classification_json, summary_text,
                c_score, s_score, j_score, d_score,
                value_score, quality_score,
                review_required, review_status, final_status, created_at, updated_at
             ) VALUES (
                ?1, ?2, 'qwen-plus', 'gpt-5-mini',
                ?3, ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, ?13, ?14, ?14
             )
             ON CONFLICT(normalized_item_id) DO UPDATE SET
                classification_json=excluded.classification_json,
                summary_text=excluded.summary_text,
                c_score=excluded.c_score,
                s_score=excluded.s_score,
                j_score=excluded.j_score,
                d_score=excluded.d_score,
                value_score=excluded.value_score,
                quality_score=excluded.quality_score,
                review_required=excluded.review_required,
                review_status=excluded.review_status,
                final_status=excluded.final_status,
                updated_at=excluded.updated_at",
            params![
                &analysis_result_id,
                &normalized_item_id,
                &classification_json,
                &analyzed.summary,
                metrics.c,
                metrics.s,
                metrics.j,
                metrics.d,
                analyzed.value_score,
                analyzed.quality_score,
                if review_required { 1 } else { 0 },
                if review_required {
                    "pending"
                } else {
                    "skipped"
                },
                final_status,
                &now
            ],
        )?;

        let exists_any: i64 = tx.query_row(
            "SELECT COUNT(1) FROM review_queue WHERE analysis_result_id = ?1",
            params![&analysis_result_id],
            |row| row.get(0),
        )?;
        let mut review_queued = false;
        if review_required && (self.budget.review_enabled() || force_review) {
            let exists_active: i64 = tx.query_row(
                "SELECT COUNT(1) FROM review_queue WHERE analysis_result_id = ?1 AND state IN ('pending','processing')",
                params![&analysis_result_id],
                |row| row.get(0),
            )?;
            if exists_active == 0 {
                review_queued = true;
                if exists_any > 0 {
                    tx.execute(
                        "UPDATE review_queue
                         SET state='pending',
                             reason=?2,
                             updated_at=?3
                         WHERE analysis_result_id=?1",
                        params![&analysis_result_id, review_reason, &now],
                    )?;
                } else {
                    tx.execute(
                        "INSERT INTO review_queue(
                            id, normalized_item_id, analysis_result_id, priority, reason, state, created_at, updated_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?6)",
                        params![
                            format!("rvw_{}", dedupe_key),
                            &normalized_item_id,
                            &analysis_result_id,
                            analyzed.priority,
                            review_reason,
                            &now
                        ],
                    )?;
                }
            }
        } else if source == "xhs" {
            if exists_any == 0 {
                tx.execute(
                    "INSERT INTO review_queue(
                        id, normalized_item_id, analysis_result_id, priority, reason, state, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 'done', ?6, ?6)",
                    params![
                        format!("rvw_{}", dedupe_key),
                        &normalized_item_id,
                        &analysis_result_id,
                        analyzed.priority,
                        "XHS auto-pass normal item",
                        &now
                    ],
                )?;
            } else {
                tx.execute(
                    "UPDATE review_queue
                     SET state='done',
                         reason=?2,
                         updated_at=?3
                     WHERE analysis_result_id=?1
                       AND state='pending'",
                    params![&analysis_result_id, "XHS auto-pass normal item", &now],
                )?;
            }
        }

        let notion_record = NotionRecord {
            normalized_item_id: normalized_item_id.clone(),
            title: text_clean.chars().take(32).collect::<String>(),
            summary: analyzed.summary.clone(),
            labels: analyzed.labels.clone(),
            source: source.clone(),
            source_url: Some(canonical_url.clone()),
            published_at: published_at.clone(),
            confidence: analyzed.classification_confidence,
            quality_score: analyzed.quality_score,
            value_score: analyzed.value_score,
        };
        let existing_sync_id: Option<String> = tx
            .query_row(
                "SELECT id FROM sync_records WHERE target = 'notion' AND normalized_item_id = ?1",
                params![&normalized_item_id],
                |row| row.get(0),
            )
            .optional()?;
        let notion_action = if existing_sync_id.is_some() {
            UpsertAction::Update
        } else {
            UpsertAction::Create
        };
        let _ = build_upsert_plan(&[notion_record], &HashMap::new());
        tx.execute(
            "INSERT INTO sync_records(
                id, normalized_item_id, target, target_record_id, sync_state, retry_count,
                next_retry_at, last_error_code, last_error_message, last_synced_at, created_at, updated_at
             ) VALUES (?1, ?2, 'notion', ?3, 'pending', 0, NULL, NULL, NULL, NULL, ?4, ?4)
             ON CONFLICT(target, normalized_item_id) DO UPDATE SET
                sync_state='pending',
                retry_count=0,
                next_retry_at=NULL,
                last_error_code=NULL,
                last_error_message=NULL,
                updated_at=excluded.updated_at",
            params![
                existing_sync_id.unwrap_or(sync_record_id.clone()),
                &normalized_item_id,
                format!("ntn_{}", &dedupe_key.chars().take(12).collect::<String>()),
                &now
            ],
        )?;
        let publish_task_id = format!("pt_{}", &source_item_id);
        let publish_blocked_by_review = source == "xhs" && review_queued;
        tx.execute(
            "INSERT INTO publish_tasks(
                id, item_id, state, attempt_count, next_retry_at, error_code, error_message, created_at, updated_at
             )
             SELECT
                ?2 AS id,
                ?3 AS item_id,
                CASE
                  WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'ignored'
                  WHEN ?5 = 1 THEN 'ignored'
                  ELSE 'pending'
                END AS state,
                0 AS attempt_count,
                NULL AS next_retry_at,
                CASE
                  WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source_inactive'
                  WHEN ?5 = 1 THEN 'await_review'
                  ELSE NULL
                END AS error_code,
                CASE
                  WHEN COALESCE(ss.active_state, 'active') = 'inactive' THEN 'source marked inactive'
                  WHEN ?5 = 1 THEN 'await manual review'
                  ELSE NULL
                END AS error_message,
                ?1 AS created_at,
                ?1 AS updated_at
             FROM normalized_items n
             LEFT JOIN source_state ss ON ss.item_id = n.source_item_id
             WHERE n.id = ?4
             ON CONFLICT(id) DO UPDATE SET
                state=excluded.state,
                attempt_count=0,
                next_retry_at=NULL,
                error_code=excluded.error_code,
                error_message=excluded.error_message,
                updated_at=excluded.updated_at",
            params![
                &now,
                &publish_task_id,
                &source_item_id,
                &normalized_item_id,
                if publish_blocked_by_review { 1 } else { 0 }
            ],
        )?;
        tx.commit()?;

        Ok(PipelineExecuteResult {
            source_item_id,
            normalized_item_id,
            analysis_result_id,
            review_queued,
            sync_record_id,
            notion_action: match notion_action {
                UpsertAction::Create => "create".to_string(),
                UpsertAction::Update => "update".to_string(),
                UpsertAction::Skip => "skip".to_string(),
            },
        })
    }

    fn record_dead_letter_with_payload(
        &self,
        conn: &Connection,
        stage: &str,
        entity_id: &str,
        payload_json: &str,
        err: &BackendError,
    ) -> Result<()> {
        self.record_dead_letter_raw(
            conn,
            stage,
            entity_id,
            payload_json,
            err.code().as_str(),
            &err.to_string(),
        )
    }

    pub(super) fn record_dead_letter_raw(
        &self,
        conn: &Connection,
        stage: &str,
        entity_id: &str,
        payload_json: &str,
        error_code: &str,
        error_message: &str,
    ) -> Result<()> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        conn.execute(
            "INSERT INTO dead_letter(
                id, entity_type, entity_id, stage, payload_json, error_code, error_message, attempt_count, state, created_at
             ) VALUES (?1, 'pipeline', ?2, ?3, ?4, ?5, ?6, 1, 'open', ?7)",
            params![
                format!(
                    "dlq_{:x}",
                    super::stable_hash(&format!(
                        "{stage}|{entity_id}|{error_code}|{error_message}"
                    ))
                ),
                entity_id,
                stage,
                payload_json,
                error_code,
                error_message,
                now
            ],
        )?;
        Ok(())
    }
}
