use helper_backend::ai_orchestrator::router::select_route;
use helper_backend::budget_guard::{BudgetStatus, CircuitState};

#[test]
fn route_to_reviewer_when_triggered_and_budget_ok() {
    let budget = BudgetStatus {
        month: "2026-02".to_string(),
        limit_cny: 100.0,
        used_cny: 50.0,
    };
    let route = select_route(0.77, 0.80, &budget);
    assert_eq!(route.primary_model, "qwen-plus");
    assert_eq!(route.review_model, Some("gpt-5-mini"));
}

#[test]
fn disable_review_when_budget_above_85_percent() {
    let budget = BudgetStatus {
        month: "2026-02".to_string(),
        limit_cny: 100.0,
        used_cny: 86.0,
    };
    assert_eq!(budget.circuit_state(), CircuitState::ReviewOff);
    let route = select_route(0.77, 0.90, &budget);
    assert_eq!(route.review_model, None);
}

#[test]
fn disable_daily_report_when_budget_above_95_percent() {
    let budget = BudgetStatus {
        month: "2026-02".to_string(),
        limit_cny: 100.0,
        used_cny: 95.0,
    };
    assert_eq!(budget.circuit_state(), CircuitState::DailyReportOff);
    assert!(!budget.daily_report_enabled());
}
