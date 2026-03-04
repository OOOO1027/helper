use helper_backend::app_core::{AppCore, PipelinePreviewReq};

#[test]
fn preview_pipeline_returns_end_to_end_shape() {
    let app = AppCore::default();
    let out = app
        .preview_pipeline(PipelinePreviewReq {
            source: "wechat".to_string(),
            source_url: Some("https://example.com/abc?from=wx".to_string()),
            published_at: Some("2026-02-25T21:10:00+08:00".to_string()),
            content_raw: "Rust + Tauri 后端方案，写入 Notion 展示".to_string(),
        })
        .expect("preview should pass");

    assert!(!out.dedupe_key.is_empty());
    assert!(!out.labels.is_empty());
    assert!(!out.summary.is_empty());
    assert!(matches!(
        out.notion_action.as_str(),
        "create" | "update" | "skip"
    ));
}
