use serde::{Deserialize, Serialize};

use crate::budget_guard::BudgetStatus;
use crate::pipeline::score::should_trigger_review;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRoute {
    pub primary_model: &'static str,
    pub review_model: Option<&'static str>,
    pub reason: String,
}

// 模型策略：Qwen-Plus 主流程，GPT-5 mini 复核
pub fn select_route(c: f64, value_score: f64, budget: &BudgetStatus) -> ModelRoute {
    let review_by_rule = should_trigger_review(c, value_score);
    if review_by_rule && budget.review_enabled() {
        return ModelRoute {
            primary_model: "qwen-plus",
            review_model: Some("gpt-5-mini"),
            reason: "review triggered by score rule".to_string(),
        };
    }

    if review_by_rule && !budget.review_enabled() {
        return ModelRoute {
            primary_model: "qwen-plus",
            review_model: None,
            reason: "review disabled by budget circuit breaker".to_string(),
        };
    }

    ModelRoute {
        primary_model: "qwen-plus",
        review_model: None,
        reason: "primary only".to_string(),
    }
}
