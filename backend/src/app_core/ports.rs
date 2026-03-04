use crate::Result;

use super::{PageReq, Paged, ReviewItem};

#[derive(Debug, Clone)]
pub struct DashboardSnapshot {
    pub today_collected: u32,
    pub pending_review: u32,
    pub classification_accuracy: f64,
    pub summary_usability: f64,
    pub used_from_ledger: f64,
}

pub trait CoreDataPort {
    fn collect_counts(
        &self,
        source: &str,
        since: Option<&str>,
        until: Option<&str>,
    ) -> Result<(u32, u32)>;

    fn dashboard_snapshot(&self, today: &str, month: &str) -> Result<DashboardSnapshot>;

    fn list_review_items(
        &self,
        state: Option<&str>,
        min_priority: Option<f64>,
        page: PageReq,
    ) -> Result<Paged<ReviewItem>>;

    fn get_review_item(&self, id: &str) -> Result<ReviewItem>;

    fn update_review_item(
        &self,
        id: &str,
        state: Option<&str>,
        reason: Option<&str>,
        updated_at: &str,
    ) -> Result<bool>;

    fn retry_review_items(&self, ids: &[String], updated_at: &str) -> Result<(u32, u32)>;
}
