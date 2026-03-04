use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::sync_notion::client::{
    map_to_notion_properties, NotionClient, NotionRecord, SyncResult,
};
use crate::{BackendError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotionPageSnapshot {
    pub page_id: String,
    pub normalized_item_id: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotionChildPage {
    pub page_id: String,
    pub title: String,
}

#[async_trait]
pub trait NotionTreeClient: Send + Sync {
    async fn list_child_pages(&self, parent_page_id: &str) -> Result<Vec<NotionChildPage>>;
    async fn create_child_page(&self, parent_page_id: &str, title: &str) -> Result<String>;
    async fn move_page(&self, page_id: &str, new_parent_id: &str) -> Result<()>;
    async fn update_page_title(&self, page_id: &str, title: &str) -> Result<()>;
    async fn append_blocks(&self, page_id: &str, blocks: &[Value]) -> Result<Vec<String>>;
    async fn update_paragraph_block(&self, block_id: &str, text: &str) -> Result<()>;
}

pub struct NotionHttpClient {
    client: reqwest::Client,
    database_id: Option<String>,
}

impl NotionHttpClient {
    pub fn new(token: String, database_id: String) -> Result<Self> {
        if token.trim().is_empty() || database_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "NOTION_TOKEN and NOTION_DATABASE_ID are required".to_string(),
            ));
        }
        let client = build_http_client(token.trim())?;
        Ok(Self {
            client,
            database_id: Some(database_id.trim().to_string()),
        })
    }

    pub fn new_page_tree(token: String) -> Result<Self> {
        if token.trim().is_empty() {
            return Err(BackendError::Validation(
                "NOTION_TOKEN is required".to_string(),
            ));
        }
        let client = build_http_client(token.trim())?;
        Ok(Self {
            client,
            database_id: None,
        })
    }

    fn database_id(&self) -> Result<&str> {
        self.database_id
            .as_deref()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| {
                BackendError::Validation(
                    "NOTION_DATABASE_ID is required for database mode".to_string(),
                )
            })
    }

    async fn parse_json_response(&self, resp: reqwest::Response, action: &str) -> Result<Value> {
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| BackendError::Internal(format!("notion {action} decode failed: {e}")))?;
        if !status.is_success() {
            return Err(BackendError::Internal(format!(
                "notion {action} non-success {status}: {text}"
            )));
        }
        serde_json::from_str(&text)
            .map_err(|e| BackendError::Internal(format!("notion {action} json parse failed: {e}")))
    }

    pub async fn list_child_pages(&self, parent_page_id: &str) -> Result<Vec<NotionChildPage>> {
        if parent_page_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "parent_page_id must not be empty".to_string(),
            ));
        }
        let url = format!(
            "https://api.notion.com/v1/blocks/{}/children",
            parent_page_id.trim()
        );
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut req = self.client.get(&url).query(&[("page_size", "100")]);
            if let Some(ref start_cursor) = cursor {
                req = req.query(&[("start_cursor", start_cursor.as_str())]);
            }
            let resp = req.send().await.map_err(|e| {
                BackendError::Internal(format!("notion list child pages failed: {e}"))
            })?;
            let v = self.parse_json_response(resp, "list child pages").await?;
            if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
                out.extend(parse_child_pages_from_results(results));
            }
            let has_more = v.get("has_more").and_then(|x| x.as_bool()).unwrap_or(false);
            if !has_more {
                break;
            }
            cursor = v
                .get("next_cursor")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    pub async fn create_child_page(&self, parent_page_id: &str, title: &str) -> Result<String> {
        if parent_page_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "parent_page_id must not be empty".to_string(),
            ));
        }
        let url = "https://api.notion.com/v1/pages";
        let body = json!({
          "parent": { "page_id": parent_page_id.trim() },
          "properties": {
            "title": {
              "title": [rich_text_item(&sanitize_page_title(title))]
            }
          }
        });
        let resp = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion create child page failed: {e}")))?;
        let v = self.parse_json_response(resp, "create child page").await?;
        v.get("id")
            .and_then(|x| x.as_str())
            .map(|v| v.to_string())
            .ok_or_else(|| {
                BackendError::Internal("notion create child page missing id".to_string())
            })
    }

    pub async fn move_page(&self, page_id: &str, new_parent_id: &str) -> Result<()> {
        if page_id.trim().is_empty() || new_parent_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "page_id and new_parent_id must not be empty".to_string(),
            ));
        }
        let url = format!("https://api.notion.com/v1/pages/{}", page_id.trim());
        let body = json!({
          "parent": { "page_id": new_parent_id.trim() }
        });
        let resp = self
            .client
            .patch(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion move page failed: {e}")))?;
        let _ = self.parse_json_response(resp, "move page").await?;
        Ok(())
    }

    pub async fn update_page_title(&self, page_id: &str, title: &str) -> Result<()> {
        if page_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "page_id must not be empty".to_string(),
            ));
        }
        let url = format!("https://api.notion.com/v1/pages/{}", page_id.trim());
        let body = json!({
          "properties": {
            "title": {
              "title": [rich_text_item(&sanitize_page_title(title))]
            }
          }
        });
        let resp = self
            .client
            .patch(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion update title failed: {e}")))?;
        let _ = self.parse_json_response(resp, "update title").await?;
        Ok(())
    }

    pub async fn append_blocks(&self, page_id: &str, blocks: &[Value]) -> Result<Vec<String>> {
        if page_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "page_id must not be empty".to_string(),
            ));
        }
        if blocks.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!(
            "https://api.notion.com/v1/blocks/{}/children",
            page_id.trim()
        );
        let body = json!({
          "children": blocks
        });
        let resp = self
            .client
            .patch(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion append blocks failed: {e}")))?;
        let v = self.parse_json_response(resp, "append blocks").await?;
        let mut ids = Vec::new();
        if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
            for block in results {
                if let Some(id) = block.get("id").and_then(|x| x.as_str()) {
                    ids.push(id.to_string());
                }
            }
        }
        Ok(ids)
    }

    pub async fn update_paragraph_block(&self, block_id: &str, text: &str) -> Result<()> {
        if block_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "block_id must not be empty".to_string(),
            ));
        }
        let url = format!("https://api.notion.com/v1/blocks/{}", block_id.trim());
        let body = json!({
          "paragraph": {
            "rich_text": [rich_text_item(text)]
          }
        });
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let resp = self
            .client
            .patch(url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion update block failed: {e}")))?;
        let _ = self.parse_json_response(resp, "update block").await?;
        Ok(())
    }

    pub async fn get_page_title(&self, page_id: &str) -> Result<String> {
        if page_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "page_id must not be empty".to_string(),
            ));
        }
        let url = format!("https://api.notion.com/v1/pages/{}", page_id.trim());
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion get page failed: {e}")))?;
        let v = self.parse_json_response(resp, "get page").await?;
        parse_page_title_from_payload(&v)
    }

    pub async fn get_paragraph_block_text(&self, block_id: &str) -> Result<String> {
        if block_id.trim().is_empty() {
            return Err(BackendError::Validation(
                "block_id must not be empty".to_string(),
            ));
        }
        let url = format!("https://api.notion.com/v1/blocks/{}", block_id.trim());
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion get block failed: {e}")))?;
        let v = self.parse_json_response(resp, "get block").await?;
        parse_paragraph_block_text(&v)
    }

    async fn query_first_page_by_normalized_id(
        &self,
        normalized_item_id: &str,
    ) -> Result<Option<Value>> {
        let database_id = self.database_id()?;
        let url = format!("https://api.notion.com/v1/databases/{}/query", database_id);
        let body = json!({
          "filter": {
            "property": "NormalizedItemID",
            "rich_text": { "equals": normalized_item_id }
          },
          "page_size": 1
        });
        let resp = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion query failed: {e}")))?;
        let v: Value = self.parse_json_response(resp, "query").await?;
        let page = v
            .get("results")
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .cloned();
        Ok(page)
    }

    async fn query_page_id_by_normalized_id(
        &self,
        normalized_item_id: &str,
    ) -> Result<Option<String>> {
        let page = self
            .query_first_page_by_normalized_id(normalized_item_id)
            .await?;
        Ok(page
            .as_ref()
            .and_then(|f| f.get("id"))
            .and_then(|id| id.as_str())
            .map(|s| s.to_string()))
    }

    pub async fn query_page_snapshot_by_normalized_id(
        &self,
        normalized_item_id: &str,
    ) -> Result<Option<NotionPageSnapshot>> {
        let page = self
            .query_first_page_by_normalized_id(normalized_item_id)
            .await?;
        Ok(page.as_ref().map(parse_page_snapshot).transpose()?)
    }

    async fn create_page(&self, record: &NotionRecord) -> Result<()> {
        let database_id = self.database_id()?;
        let url = "https://api.notion.com/v1/pages";
        let body = json!({
          "parent": { "database_id": database_id },
          "properties": map_to_notion_properties(record)
        });
        let resp = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion create failed: {e}")))?;
        let _ = self.parse_json_response(resp, "create").await?;
        Ok(())
    }

    async fn update_page(&self, page_id: &str, record: &NotionRecord) -> Result<()> {
        let url = format!("https://api.notion.com/v1/pages/{}", page_id);
        let body = json!({
          "properties": map_to_notion_properties(record)
        });
        let resp = self
            .client
            .patch(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Internal(format!("notion update failed: {e}")))?;
        let _ = self.parse_json_response(resp, "update").await?;
        Ok(())
    }
}

fn build_http_client(token: &str) -> Result<reqwest::Client> {
    let normalized_token = normalize_notion_token(token);
    if normalized_token.is_empty() {
        return Err(BackendError::Validation(
            "NOTION_TOKEN is required".to_string(),
        ));
    }
    let mut headers = HeaderMap::new();
    let auth = format!("Bearer {}", normalized_token);
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth)
            .map_err(|e| BackendError::Validation(format!("invalid notion token: {e}")))?,
    );
    headers.insert("Notion-Version", HeaderValue::from_static("2022-06-28"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| BackendError::Internal(format!("build notion client failed: {e}")))
}

fn normalize_notion_token(raw: &str) -> String {
    let mut token = raw.trim().trim_matches('"').trim_matches('\'').to_string();
    loop {
        let lower = token.to_ascii_lowercase();
        if lower.starts_with("bearer ") {
            token = token[7..].trim().to_string();
            continue;
        }
        break;
    }
    token
}

fn rich_text_item(text: &str) -> Value {
    json!({
      "type": "text",
      "text": { "content": text }
    })
}

fn sanitize_page_title(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return "Untitled".to_string();
    }
    trimmed.chars().take(120).collect::<String>()
}

fn parse_child_pages_from_results(results: &[Value]) -> Vec<NotionChildPage> {
    let mut out = Vec::new();
    for item in results {
        if item
            .get("type")
            .and_then(|v| v.as_str())
            .map(|v| v != "child_page")
            .unwrap_or(true)
        {
            continue;
        }
        let Some(page_id) = item.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let title = item
            .get("child_page")
            .and_then(|v| v.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        out.push(NotionChildPage {
            page_id: page_id.to_string(),
            title: title.to_string(),
        });
    }
    out
}

fn parse_page_snapshot(page: &Value) -> Result<NotionPageSnapshot> {
    let page_id = page
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| BackendError::Internal("notion page missing id".to_string()))?
        .to_string();
    let properties = page
        .get("properties")
        .ok_or_else(|| BackendError::Internal("notion page missing properties".to_string()))?;

    let normalized_item_id = read_text_property(properties, "NormalizedItemID");
    let title = read_title_property(properties, "Title");
    let summary = read_text_property(properties, "Summary");
    let source = properties
        .get("Source")
        .and_then(|v| v.get("select"))
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let source_url = properties
        .get("SourceURL")
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(NotionPageSnapshot {
        page_id,
        normalized_item_id,
        title,
        summary,
        source,
        source_url,
    })
}

fn parse_page_title_from_payload(page: &Value) -> Result<String> {
    let properties = page
        .get("properties")
        .ok_or_else(|| BackendError::Internal("notion page missing properties".to_string()))?;
    if let Some(map) = properties.as_object() {
        for value in map.values() {
            if let Some(title_items) = value.get("title").and_then(|v| v.as_array()) {
                let parsed = read_rich_text_items(title_items);
                if !parsed.trim().is_empty() {
                    return Ok(parsed);
                }
            }
        }
    }
    Ok(String::new())
}

fn parse_paragraph_block_text(block: &Value) -> Result<String> {
    let block_type = block
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if block_type != "paragraph" {
        return Err(BackendError::Internal(format!(
            "notion block is not paragraph: {}",
            block_type
        )));
    }
    let rich_text = block
        .get("paragraph")
        .and_then(|v| v.get("rich_text"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            BackendError::Internal("notion paragraph block missing rich_text".to_string())
        })?;
    Ok(read_rich_text_items(rich_text))
}

fn read_text_property(properties: &Value, key: &str) -> String {
    properties
        .get(key)
        .and_then(|v| v.get("rich_text"))
        .and_then(|v| v.as_array())
        .map(|items| read_rich_text_items(items))
        .unwrap_or_default()
}

fn read_title_property(properties: &Value, key: &str) -> String {
    properties
        .get(key)
        .and_then(|v| v.get("title"))
        .and_then(|v| v.as_array())
        .map(|items| read_rich_text_items(items))
        .unwrap_or_default()
}

fn read_rich_text_items(items: &[Value]) -> String {
    let mut out = String::new();
    for item in items {
        if let Some(text) = item.get("plain_text").and_then(|v| v.as_str()) {
            out.push_str(text);
            continue;
        }
        if let Some(text) = item
            .get("text")
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
        {
            out.push_str(text);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_notion_token, parse_child_pages_from_results, parse_page_snapshot,
        parse_page_title_from_payload, parse_paragraph_block_text, sanitize_page_title,
    };
    use serde_json::json;

    #[test]
    fn parse_page_snapshot_reads_expected_fields() {
        let page = json!({
            "id": "page_1",
            "properties": {
                "NormalizedItemID": {
                    "rich_text": [{ "plain_text": "norm_abc" }]
                },
                "Title": {
                    "title": [{ "plain_text": "backend title" }]
                },
                "Summary": {
                    "rich_text": [{ "plain_text": "summary text" }]
                },
                "Source": {
                    "select": { "name": "wechat" }
                },
                "SourceURL": {
                    "url": "https://example.com/x"
                }
            }
        });

        let snapshot = parse_page_snapshot(&page).expect("parse snapshot");
        assert_eq!(snapshot.page_id, "page_1");
        assert_eq!(snapshot.normalized_item_id, "norm_abc");
        assert_eq!(snapshot.title, "backend title");
        assert_eq!(snapshot.summary, "summary text");
        assert_eq!(snapshot.source, "wechat");
        assert_eq!(
            snapshot.source_url.as_deref(),
            Some("https://example.com/x")
        );
    }

    #[test]
    fn parse_child_pages_ignores_non_page_blocks() {
        let results = json!([
            {
                "id": "block_1",
                "type": "paragraph"
            },
            {
                "id": "page_1",
                "type": "child_page",
                "child_page": { "title": "Inbox" }
            }
        ]);
        let pages = parse_child_pages_from_results(results.as_array().expect("arr"));
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_id, "page_1");
        assert_eq!(pages[0].title, "Inbox");
    }

    #[test]
    fn sanitize_page_title_limits_length() {
        let raw = "  ".to_string() + &"x".repeat(500);
        let title = sanitize_page_title(&raw);
        assert_eq!(title.len(), 120);
    }

    #[test]
    fn parse_page_title_from_payload_uses_first_non_empty_title_property() {
        let payload = json!({
            "properties": {
                "Name": { "title": [{ "plain_text": "Tree Item Title" }] },
                "Other": { "rich_text": [] }
            }
        });
        let title = parse_page_title_from_payload(&payload).expect("parse page title");
        assert_eq!(title, "Tree Item Title");
    }

    #[test]
    fn parse_paragraph_block_text_reads_plain_text() {
        let block = json!({
            "type": "paragraph",
            "paragraph": {
                "rich_text": [{ "plain_text": "hello metadata" }]
            }
        });
        let text = parse_paragraph_block_text(&block).expect("parse paragraph");
        assert_eq!(text, "hello metadata");
    }

    #[test]
    fn parse_paragraph_block_text_rejects_non_paragraph() {
        let block = json!({
            "type": "heading_1",
            "heading_1": { "rich_text": [{ "plain_text": "h1" }] }
        });
        let err = parse_paragraph_block_text(&block).expect_err("should reject");
        assert!(err.to_string().contains("not paragraph"));
    }

    #[test]
    fn normalize_notion_token_removes_bearer_prefix_and_quotes() {
        assert_eq!(
            normalize_notion_token("Bearer ntn_abc123"),
            "ntn_abc123".to_string()
        );
        assert_eq!(
            normalize_notion_token("bearer   ntn_abc123"),
            "ntn_abc123".to_string()
        );
        assert_eq!(
            normalize_notion_token("\"ntn_abc123\""),
            "ntn_abc123".to_string()
        );
        assert_eq!(
            normalize_notion_token("'ntn_abc123'"),
            "ntn_abc123".to_string()
        );
    }
}

#[async_trait]
impl NotionClient for NotionHttpClient {
    async fn upsert_batch(&self, records: &[NotionRecord]) -> Result<SyncResult> {
        let mut success = 0usize;
        let mut failed = 0usize;

        for record in records {
            let maybe_page_id = self
                .query_page_id_by_normalized_id(&record.normalized_item_id)
                .await?;
            let op_result = if let Some(page_id) = maybe_page_id {
                self.update_page(&page_id, record).await
            } else {
                self.create_page(record).await
            };
            match op_result {
                Ok(()) => success += 1,
                Err(_) => failed += 1,
            }
        }

        Ok(SyncResult { success, failed })
    }
}

#[async_trait]
impl NotionTreeClient for NotionHttpClient {
    async fn list_child_pages(&self, parent_page_id: &str) -> Result<Vec<NotionChildPage>> {
        NotionHttpClient::list_child_pages(self, parent_page_id).await
    }

    async fn create_child_page(&self, parent_page_id: &str, title: &str) -> Result<String> {
        NotionHttpClient::create_child_page(self, parent_page_id, title).await
    }

    async fn move_page(&self, page_id: &str, new_parent_id: &str) -> Result<()> {
        NotionHttpClient::move_page(self, page_id, new_parent_id).await
    }

    async fn update_page_title(&self, page_id: &str, title: &str) -> Result<()> {
        NotionHttpClient::update_page_title(self, page_id, title).await
    }

    async fn append_blocks(&self, page_id: &str, blocks: &[Value]) -> Result<Vec<String>> {
        NotionHttpClient::append_blocks(self, page_id, blocks).await
    }

    async fn update_paragraph_block(&self, block_id: &str, text: &str) -> Result<()> {
        NotionHttpClient::update_paragraph_block(self, block_id, text).await
    }
}
