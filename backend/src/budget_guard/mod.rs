use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub month: String,
    pub limit_cny: f64,
    pub used_cny: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CircuitState {
    Normal,
    ReviewOff,
    DailyReportOff,
}

impl BudgetStatus {
    pub fn usage_ratio(&self) -> f64 {
        if self.limit_cny <= 0.0 {
            return 1.0;
        }
        (self.used_cny / self.limit_cny).max(0.0)
    }

    // 熔断：预算使用率 >=85% 关复核
    pub fn review_enabled(&self) -> bool {
        self.usage_ratio() < 0.85
    }

    // 熔断：预算使用率 >=95% 关日报
    pub fn daily_report_enabled(&self) -> bool {
        self.usage_ratio() < 0.95
    }

    pub fn circuit_state(&self) -> CircuitState {
        if !self.daily_report_enabled() {
            CircuitState::DailyReportOff
        } else if !self.review_enabled() {
            CircuitState::ReviewOff
        } else {
            CircuitState::Normal
        }
    }
}
