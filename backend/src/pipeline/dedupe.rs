use std::collections::HashSet;

pub const SEMANTIC_SIM_THRESHOLD: f64 = 0.90;
pub const SEMANTIC_WINDOW_DAYS: i64 = 14;

pub fn is_hard_duplicate(url_hash: &str, existing_hashes: &HashSet<String>) -> bool {
    existing_hashes.contains(url_hash)
}

// 语义去重：sim >= 0.90 且 |Δt|<=14天
pub fn is_semantic_duplicate(similarity: f64, delta_days_abs: i64) -> bool {
    similarity >= SEMANTIC_SIM_THRESHOLD && delta_days_abs <= SEMANTIC_WINDOW_DAYS
}
