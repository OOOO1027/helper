use helper_backend::pipeline::score::{
    gate_bucket, quality_score, review_priority, should_trigger_review, value_score, GateBucket,
};

fn approx_eq(a: f64, b: f64, eps: f64) {
    assert!(
        (a - b).abs() <= eps,
        "left={a}, right={b}, diff={}",
        (a - b).abs()
    );
}

#[test]
fn quality_formula_matches_baseline() {
    let score = quality_score(0.8, 0.7, 0.6, 0.9);
    approx_eq(score, 74.5, 1e-6);
}

#[test]
fn value_formula_matches_baseline() {
    let score = value_score(0.8, 0.8, 0.9, 0.7);
    approx_eq(score, 0.805, 1e-6);
}

#[test]
fn review_trigger_rule_matches_baseline() {
    assert!(should_trigger_review(0.77, 0.30));
    assert!(should_trigger_review(0.90, 0.85));
    assert!(!should_trigger_review(0.90, 0.84));
}

#[test]
fn gate_rule_matches_baseline() {
    assert_eq!(gate_bucket(78.0), GateBucket::Direct);
    assert_eq!(gate_bucket(65.0), GateBucket::Review);
    assert_eq!(gate_bucket(64.9), GateBucket::Exception);
}

#[test]
fn review_priority_matches_baseline() {
    let p = review_priority(0.8, 0.9);
    approx_eq(p, 0.48, 1e-9);
}
