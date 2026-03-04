use async_trait::async_trait;

use crate::collectors::{CollectWindow, CollectedItem, Collector};
use crate::Result;

#[derive(Debug, Clone)]
pub struct BiliCollector {
    pub enabled: bool,
}

impl Default for BiliCollector {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[async_trait]
impl Collector for BiliCollector {
    fn source(&self) -> &'static str {
        "bili"
    }

    async fn collect(&self, _window: &CollectWindow) -> Result<Vec<CollectedItem>> {
        if !self.enabled {
            return Ok(Vec::new());
        }
        Ok(Vec::new())
    }
}
