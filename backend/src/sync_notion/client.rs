use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionRecord {
    pub normalized_item_id: String,
    pub title: String,
    pub summary: String,
    pub labels: Vec<String>,
    pub source: String,
    pub source_url: Option<String>,
    pub published_at: Option<String>,
    pub confidence: f64,
    pub quality_score: f64,
    pub value_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub success: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncErrorDetail {
    pub normalized_item_id: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotentSyncResult {
    pub create: usize,
    pub update: usize,
    pub skip: usize,
    pub errors: Vec<SyncErrorDetail>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum UpsertAction {
    Create,
    Update,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertPlanItem {
    pub normalized_item_id: String,
    pub idempotency_key: String,
    pub action: UpsertAction,
    pub target_record_id: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 3 }
    }
}

impl RetryPolicy {
    // 2^n + jitter（毫秒）
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        let n = attempt.min(10);
        let jitter = (attempt as u64 * 137) % 250;
        (1u64 << n) * 1000 + jitter
    }
}

pub fn idempotency_key(record: &NotionRecord) -> String {
    let raw = format!(
        "{}|{}|{}|{:.4}|{:.4}|{:.4}",
        record.normalized_item_id,
        record.title,
        record.summary,
        record.confidence,
        record.quality_score,
        record.value_score
    );
    format!("{:x}", fnv1a_hash(raw.as_bytes()))
}

pub fn map_to_notion_properties(record: &NotionRecord) -> Value {
    json!({
      "NormalizedItemID": { "rich_text": [{ "text": { "content": record.normalized_item_id } }] },
      "Title": { "title": [{ "text": { "content": record.title } }] },
      "Summary": { "rich_text": [{ "text": { "content": record.summary } }] },
      "Labels": { "multi_select": record.labels.iter().map(|v| json!({"name": v})).collect::<Vec<_>>() },
      "Source": { "select": { "name": record.source } },
      "SourceURL": { "url": record.source_url },
      "PublishedAt": { "rich_text": [{ "text": { "content": record.published_at.clone().unwrap_or_default() } }] },
      "Confidence": { "number": record.confidence },
      "QualityScore": { "number": record.quality_score },
      "ValueScore": { "number": record.value_score }
    })
}

pub fn build_upsert_plan(
    records: &[NotionRecord],
    // key: normalized_item_id, value: (record_id, idempotency_key)
    existing_index: &HashMap<String, (String, String)>,
) -> Vec<UpsertPlanItem> {
    let mut out = Vec::with_capacity(records.len());
    for record in records {
        let key = idempotency_key(record);
        let existing = existing_index.get(&record.normalized_item_id);
        let (action, target_record_id) = match existing {
            Some((record_id, old_key)) if *old_key == key => {
                (UpsertAction::Skip, Some(record_id.clone()))
            }
            Some((record_id, _)) => (UpsertAction::Update, Some(record_id.clone())),
            None => (UpsertAction::Create, None),
        };
        out.push(UpsertPlanItem {
            normalized_item_id: record.normalized_item_id.clone(),
            idempotency_key: key,
            action,
            target_record_id,
            payload: map_to_notion_properties(record),
        });
    }
    out
}

#[async_trait]
pub trait NotionClient: Send + Sync {
    // 限流策略：每批10条 + 400ms间隔（由调用方调度）
    async fn upsert_batch(&self, records: &[NotionRecord]) -> Result<SyncResult>;

    async fn upsert_one(&self, record: &NotionRecord) -> Result<()> {
        let result = self.upsert_batch(std::slice::from_ref(record)).await?;
        if result.failed > 0 {
            return Err(crate::BackendError::Internal(
                "notion upsert_one failed".to_string(),
            ));
        }
        Ok(())
    }
}

fn fnv1a_hash(bytes: &[u8]) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
