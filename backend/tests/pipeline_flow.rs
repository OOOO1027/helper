use helper_backend::pipeline::analyze::{
    analyze, classify_failure, AnalyzeInput, ContentAnalysisInput,
};
use helper_backend::pipeline::queue::{transition, PipelineEvent, PipelineState};

#[test]
fn analyze_returns_labels_summary_confidence() {
    let out = analyze(&ContentAnalysisInput {
        metrics: AnalyzeInput {
            c: 0.82,
            s: 0.79,
            j: 0.75,
            d: 0.80,
            r: 0.70,
            f: 0.72,
            e: 0.74,
            p: 0.76,
        },
        text: "Rust + Tauri + 微信 导入到 Notion 的后端流程".to_string(),
    });

    assert!(!out.labels.is_empty());
    assert!(!out.tags.is_empty());
    assert!(!out.final_category.is_empty());
    assert!(!out.route_reason.is_empty());
    assert!(!out.key_points.is_empty());
    assert!(out.summary_confidence >= 0.0 && out.summary_confidence <= 1.0);
    assert!(out.classification_confidence >= 0.0 && out.classification_confidence <= 1.0);
}

#[test]
fn retryable_failure_routes_to_retry_then_failed() {
    let f0 = classify_failure("MOD-3001", 0);
    assert!(f0.retryable);
    assert!(f0.next_retry_ms.is_some());
    assert!(!f0.to_failed_queue);

    let f3 = classify_failure("MOD-3001", 3);
    assert!(f3.retryable);
    assert!(f3.next_retry_ms.is_none());
    assert!(f3.to_failed_queue);
}

#[test]
fn state_machine_transitions() {
    let s1 = transition(PipelineState::Queued, PipelineEvent::Start);
    assert_eq!(s1, PipelineState::Processing);
    let s2 = transition(s1, PipelineEvent::TemporaryFailure);
    assert_eq!(s2, PipelineState::RetryWait);
    let s3 = transition(s2, PipelineEvent::RetryReady);
    assert_eq!(s3, PipelineState::Queued);
}
