use std::collections::HashMap;

use helper_backend::sync_notion::client::{
    build_upsert_plan, idempotency_key, NotionRecord, UpsertAction,
};

fn sample_record() -> NotionRecord {
    NotionRecord {
        normalized_item_id: "nid-1".to_string(),
        title: "Rust pipeline".to_string(),
        summary: "summary".to_string(),
        labels: vec!["后端工程".to_string()],
        source: "wechat".to_string(),
        source_url: Some("https://example.com/a".to_string()),
        published_at: Some("2026-02-25 21:30:00".to_string()),
        confidence: 0.82,
        quality_score: 81.0,
        value_score: 0.76,
    }
}

#[test]
fn upsert_plan_create_when_missing() {
    let record = sample_record();
    let plan = build_upsert_plan(&[record], &HashMap::new());
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].action, UpsertAction::Create);
}

#[test]
fn upsert_plan_skip_when_idempotency_key_same() {
    let record = sample_record();
    let key = idempotency_key(&record);
    let mut index = HashMap::new();
    index.insert("nid-1".to_string(), ("notion-page-1".to_string(), key));

    let plan = build_upsert_plan(&[record], &index);
    assert_eq!(plan[0].action, UpsertAction::Skip);
    assert_eq!(plan[0].target_record_id.as_deref(), Some("notion-page-1"));
}
