use std::collections::{HashMap, HashSet};
use std::thread::sleep;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::collectors::{CollectWindow, CollectedItem, Collector};
use crate::{BackendError, Result};

use utils::{
    ensure_payload_ok, extract_page, read_env_u32, read_env_u64, should_fallback_to_chrome,
    should_fallback_to_profile_scrape, stable_hash, within_window,
};

mod cookie_api;
mod chrome_inject;
mod profile_scraper;
mod utils;

pub(super) const XHS_BASE_URL: &str = "https://www.xiaohongshu.com";
const DEFAULT_PAGE_SIZE: u32 = 20;
const DEFAULT_MAX_PAGES: u32 = 120;
const DEFAULT_MIN_DELAY_MS: u64 = 1500;
const DEFAULT_MAX_DELAY_MS: u64 = 3000;
const DEFAULT_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub struct XhsCollector {
    pub cursor: Option<String>,
    page_size: u32,
    max_pages: u32,
    min_delay_ms: u64,
    max_delay_ms: u64,
    pub(super) user_agent: String,
}

impl Default for XhsCollector {
    fn default() -> Self {
        Self {
            cursor: None,
            page_size: read_env_u32("XHS_PAGE_SIZE", DEFAULT_PAGE_SIZE).max(1),
            max_pages: read_env_u32("XHS_MAX_PAGES", DEFAULT_MAX_PAGES).max(1),
            min_delay_ms: read_env_u64("XHS_MIN_DELAY_MS", DEFAULT_MIN_DELAY_MS),
            max_delay_ms: read_env_u64("XHS_MAX_DELAY_MS", DEFAULT_MAX_DELAY_MS),
            user_agent: std::env::var("XHS_USER_AGENT")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_UA.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum FeedKind {
    Likes,
    Favorites,
}

impl FeedKind {
    fn path(self) -> &'static str {
        match self {
            FeedKind::Likes => "/api/sns/web/v1/you/likes",
            FeedKind::Favorites => "/web_api/sns/v1/file/faved/list",
        }
    }
}

#[derive(Debug, Default)]
pub(self) struct PageExtract {
    items: Vec<CollectedItem>,
    has_more: Option<bool>,
    next_cursor: Option<String>,
}

#[async_trait]
impl Collector for XhsCollector {
    fn source(&self) -> &'static str {
        "xhs"
    }

    async fn collect(&self, window: &CollectWindow) -> Result<Vec<CollectedItem>> {
        self.collect_internal(window, &HashSet::new()).await
    }
}

impl XhsCollector {
    pub async fn collect_internal(
        &self,
        window: &CollectWindow,
        stop_external_ids: &HashSet<String>,
    ) -> Result<Vec<CollectedItem>> {
        if let Ok(raw) = std::env::var("HELPER_XHS_FAKE_ITEMS_JSON") {
            let mut mock_items: Vec<CollectedItem> = serde_json::from_str(&raw).map_err(|e| {
                BackendError::Validation(format!("invalid HELPER_XHS_FAKE_ITEMS_JSON: {e}"))
            })?;
            mock_items.retain(|item| !item.external_id.trim().is_empty());
            return Ok(mock_items);
        }

        let mut all = Vec::new();
        let mut seen = HashSet::new();

        let api_result = async {
            self.collect_feed(
                FeedKind::Likes,
                window,
                stop_external_ids,
                &mut seen,
                &mut all,
            )
            .await?;
            self.collect_feed(
                FeedKind::Favorites,
                window,
                stop_external_ids,
                &mut seen,
                &mut all,
            )
            .await?;
            Ok::<(), BackendError>(())
        }
        .await;

        match api_result {
            Ok(()) => Ok(all),
            Err(err) if should_fallback_to_profile_scrape(&err) => {
                self.collect_from_profile_tabs(window, stop_external_ids)
                    .await
            }
            Err(err) => Err(err),
        }
    }

    async fn collect_feed(
        &self,
        feed: FeedKind,
        window: &CollectWindow,
        stop_external_ids: &HashSet<String>,
        seen: &mut HashSet<String>,
        out: &mut Vec<CollectedItem>,
    ) -> Result<()> {
        let mut cursor = self.cursor.clone().unwrap_or_default();
        let mut page = 1u32;
        for index in 0..self.max_pages {
            let mut params = HashMap::new();
            match feed {
                FeedKind::Likes => {
                    params.insert("num".to_string(), self.page_size.to_string());
                    params.insert("cursor".to_string(), cursor.clone());
                }
                FeedKind::Favorites => {
                    params.insert("page".to_string(), page.to_string());
                    params.insert("page_size".to_string(), self.page_size.to_string());
                }
            }

            let payload = self.fetch_page_json(feed.path(), &params).await?;
            ensure_payload_ok(&payload)?;
            let extracted = extract_page(&payload);
            let page_item_count = extracted.items.len() as u32;
            if page_item_count == 0 {
                break;
            }

            let mut should_stop = false;
            for item in extracted.items {
                if stop_external_ids.contains(&item.external_id) {
                    should_stop = true;
                    break;
                }
                if !seen.insert(item.external_id.clone()) {
                    continue;
                }
                if within_window(window, item.published_at.as_deref()) {
                    out.push(item);
                }
            }
            if should_stop {
                break;
            }

            match feed {
                FeedKind::Likes => {
                    let has_more = extracted
                        .has_more
                        .unwrap_or(out.len() as u32 >= self.page_size);
                    let next_cursor = extracted.next_cursor.unwrap_or_default();
                    if !has_more || next_cursor.is_empty() || next_cursor == cursor {
                        break;
                    }
                    cursor = next_cursor;
                }
                FeedKind::Favorites => {
                    let has_more = extracted
                        .has_more
                        .unwrap_or(page_item_count >= self.page_size);
                    if !has_more {
                        break;
                    }
                    page += 1;
                }
            }

            sleep(Duration::from_millis(self.delay_for(index)));
        }
        Ok(())
    }

    async fn fetch_page_json(
        &self,
        path: &str,
        params: &HashMap<String, String>,
    ) -> Result<Value> {
        if let Some(cookie) = self.resolve_cookie() {
            match self.fetch_with_cookie(path, params, &cookie).await {
                Ok(payload) => Ok(payload),
                Err(err) if should_fallback_to_chrome(&err) => {
                    self.fetch_with_chrome(path, params).await
                }
                Err(err) => Err(err),
            }
        } else {
            self.fetch_with_chrome(path, params).await
        }
    }

    fn delay_for(&self, page_index: u32) -> u64 {
        let min = self.min_delay_ms.min(self.max_delay_ms);
        let max = self.max_delay_ms.max(self.min_delay_ms);
        if min == max {
            return min;
        }
        let span = max - min;
        min + (stable_hash(&format!("xhs-delay-{page_index}")) % (span + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_page_parses_like_payload() {
        let payload = json!({
            "success": true,
            "code": 0,
            "data": {
                "has_more": true,
                "cursor": "next-c",
                "items": [
                    {
                        "id": "abc123",
                        "xsec_token": "token",
                        "display_title": "标题A",
                        "desc": "内容A",
                        "publish_time": 1730000000,
                        "cover": {"url_default": "https://img.example.com/cover.jpg"},
                        "image_list": [
                            {"url_default": "https://img.example.com/1.jpg"},
                            {"url_default": "https://img.example.com/2.jpg"}
                        ]
                    }
                ]
            }
        });

        let extracted = utils::extract_page(&payload);
        assert_eq!(extracted.items.len(), 1);
        assert_eq!(extracted.items[0].external_id, "abc123");
        assert!(extracted.items[0].source_url.contains("abc123"));
        assert_eq!(
            extracted.items[0].cover_url.as_deref(),
            Some("https://img.example.com/cover.jpg")
        );
        assert_eq!(extracted.items[0].image_urls.len(), 3);
        assert_eq!(extracted.next_cursor.as_deref(), Some("next-c"));
        assert_eq!(extracted.has_more, Some(true));
    }

    #[test]
    fn normalize_ts_supports_epoch_millis() {
        let payload = json!({
            "success": true,
            "code": 0,
            "data": {
                "items": [{
                    "id": "ts1",
                    "xsec_token": "tok",
                    "title": "t",
                    "publish_time": "1730000000000"
                }]
            }
        });
        let extracted = utils::extract_page(&payload);
        let ts = extracted.items[0].published_at.as_deref().expect("ts");
        assert!(ts.starts_with("2024-"));
    }

    #[test]
    fn collector_reads_fake_items_from_env() {
        let _guard = ENV_LOCK.lock().expect("lock");
        std::env::set_var(
            "HELPER_XHS_FAKE_ITEMS_JSON",
            r#"[{"external_id":"x1","source_url":"https://www.xiaohongshu.com/explore/x1","title":"t","content_raw":"c","published_at":"2026-02-26 10:00:00"}]"#,
        );
        let collector = XhsCollector::default();
        let items = futures::executor::block_on(collector.collect(&CollectWindow {
            since: None,
            until: None,
        }))
        .expect("collect");
        std::env::remove_var("HELPER_XHS_FAKE_ITEMS_JSON");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].external_id, "x1");
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
