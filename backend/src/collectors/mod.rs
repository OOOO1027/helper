pub mod bili;
pub mod wechat_import;
pub mod xhs;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectWindow {
    pub since: Option<String>,
    pub until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedItem {
    pub external_id: String,
    pub source_url: String,
    pub title: Option<String>,
    pub content_raw: String,
    pub published_at: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
}

#[async_trait]
pub trait Collector: Send + Sync {
    fn source(&self) -> &'static str;
    async fn collect(&self, window: &CollectWindow) -> Result<Vec<CollectedItem>>;
}
