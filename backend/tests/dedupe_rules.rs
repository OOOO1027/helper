use std::collections::HashSet;

use helper_backend::pipeline::dedupe::{is_hard_duplicate, is_semantic_duplicate};

#[test]
fn hard_duplicate_by_url_hash() {
    let mut hashes = HashSet::new();
    hashes.insert("abc".to_string());
    assert!(is_hard_duplicate("abc", &hashes));
    assert!(!is_hard_duplicate("def", &hashes));
}

#[test]
fn semantic_duplicate_within_14_days_and_high_similarity() {
    assert!(is_semantic_duplicate(0.90, 14));
    assert!(!is_semantic_duplicate(0.89, 14));
    assert!(!is_semantic_duplicate(0.95, 15));
}
