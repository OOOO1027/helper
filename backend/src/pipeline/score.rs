#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateBucket {
    Direct,
    Review,
    Exception,
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

// QualityScore = 100*(0.40*C + 0.35*S + 0.15*J + 0.10*D)
pub fn quality_score(c: f64, s: f64, j: f64, d: f64) -> f64 {
    100.0 * (0.40 * clamp01(c) + 0.35 * clamp01(s) + 0.15 * clamp01(j) + 0.10 * clamp01(d))
}

// ValueScore = 0.30*R + 0.25*F + 0.25*E + 0.20*P
pub fn value_score(r: f64, f: f64, e: f64, p: f64) -> f64 {
    0.30 * clamp01(r) + 0.25 * clamp01(f) + 0.25 * clamp01(e) + 0.20 * clamp01(p)
}

// 复核触发：C < 0.78 OR ValueScore >= 0.85
pub fn should_trigger_review(c: f64, value_score: f64) -> bool {
    clamp01(c) < 0.78 || clamp01(value_score) >= 0.85
}

// 入库闸门：>=78 直入；65~77 待审；<65 异常
pub fn gate_bucket(quality_score: f64) -> GateBucket {
    if quality_score >= 78.0 {
        GateBucket::Direct
    } else if quality_score >= 65.0 {
        GateBucket::Review
    } else {
        GateBucket::Exception
    }
}

// 审核排序：Priority = 0.6*(1-C) + 0.4*ValueScore
pub fn review_priority(c: f64, value_score: f64) -> f64 {
    0.6 * (1.0 - clamp01(c)) + 0.4 * clamp01(value_score)
}
