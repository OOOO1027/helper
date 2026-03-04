use async_trait::async_trait;
use futures::executor::block_on;
use helper_backend::storage::db::run_migrations;
use helper_backend::sync_notion::notion_api::{NotionChildPage, NotionTreeClient};
use helper_backend::sync_notion::service::sync_pending_page_tree_with_conn_with_options;
use helper_backend::{BackendError, Result};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Default)]
struct MockTreeState {
    next_page: u64,
    next_block: u64,
    children: HashMap<String, Vec<NotionChildPage>>,
    page_parent: HashMap<String, String>,
    page_title: HashMap<String, String>,
    create_calls: Vec<(String, String)>,
    move_calls: Vec<(String, String)>,
    append_calls: Vec<(String, usize)>,
    appended_paragraph_texts: Vec<String>,
    update_block_calls: usize,
}

struct MockNotionTreeClient {
    state: Mutex<MockTreeState>,
    fail_on_title: Option<String>,
    fail_message: Option<String>,
}

impl MockNotionTreeClient {
    fn new(fail_on_title: Option<&str>) -> Self {
        Self {
            state: Mutex::new(MockTreeState {
                next_page: 1,
                next_block: 1,
                ..MockTreeState::default()
            }),
            fail_on_title: fail_on_title.map(|v| v.to_string()),
            fail_message: None,
        }
    }

    fn new_with_error(fail_on_title: Option<&str>, fail_message: &str) -> Self {
        Self {
            state: Mutex::new(MockTreeState {
                next_page: 1,
                next_block: 1,
                ..MockTreeState::default()
            }),
            fail_on_title: fail_on_title.map(|v| v.to_string()),
            fail_message: Some(fail_message.to_string()),
        }
    }

    fn snapshot(&self) -> MockTreeState {
        self.state.lock().expect("lock state").clone()
    }
}

fn extract_paragraph_text(block: &Value) -> Option<String> {
    block
        .get("paragraph")
        .and_then(|v| v.get("rich_text"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("text"))
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

#[async_trait]
impl NotionTreeClient for MockNotionTreeClient {
    async fn list_child_pages(&self, parent_page_id: &str) -> Result<Vec<NotionChildPage>> {
        let state = self.state.lock().expect("lock state");
        Ok(state
            .children
            .get(parent_page_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn create_child_page(&self, parent_page_id: &str, title: &str) -> Result<String> {
        if self
            .fail_on_title
            .as_deref()
            .map(|v| v == title)
            .unwrap_or(false)
        {
            let fail_message = self
                .fail_message
                .clone()
                .unwrap_or_else(|| "mock create page failed".to_string());
            return Err(BackendError::Internal(fail_message));
        }

        let mut state = self.state.lock().expect("lock state");
        let page_id = format!("page_{}", state.next_page);
        state.next_page += 1;
        state
            .page_parent
            .insert(page_id.clone(), parent_page_id.to_string());
        state.page_title.insert(page_id.clone(), title.to_string());
        state
            .children
            .entry(parent_page_id.to_string())
            .or_default()
            .push(NotionChildPage {
                page_id: page_id.clone(),
                title: title.to_string(),
            });
        state
            .create_calls
            .push((parent_page_id.to_string(), title.to_string()));
        Ok(page_id)
    }

    async fn move_page(&self, page_id: &str, new_parent_id: &str) -> Result<()> {
        let mut state = self.state.lock().expect("lock state");
        let old_parent = state
            .page_parent
            .get(page_id)
            .cloned()
            .ok_or_else(|| BackendError::Internal("mock page not found".to_string()))?;
        let title = state
            .page_title
            .get(page_id)
            .cloned()
            .unwrap_or_else(|| "Untitled".to_string());

        if let Some(old_children) = state.children.get_mut(&old_parent) {
            old_children.retain(|p| p.page_id != page_id);
        }
        state
            .children
            .entry(new_parent_id.to_string())
            .or_default()
            .push(NotionChildPage {
                page_id: page_id.to_string(),
                title,
            });
        state
            .page_parent
            .insert(page_id.to_string(), new_parent_id.to_string());
        state
            .move_calls
            .push((page_id.to_string(), new_parent_id.to_string()));
        Ok(())
    }

    async fn update_page_title(&self, page_id: &str, title: &str) -> Result<()> {
        let mut state = self.state.lock().expect("lock state");
        let parent = state
            .page_parent
            .get(page_id)
            .cloned()
            .ok_or_else(|| BackendError::Internal("mock page not found".to_string()))?;
        state
            .page_title
            .insert(page_id.to_string(), title.to_string());
        if let Some(children) = state.children.get_mut(&parent) {
            if let Some(child) = children.iter_mut().find(|p| p.page_id == page_id) {
                child.title = title.to_string();
            }
        }
        Ok(())
    }

    async fn append_blocks(&self, page_id: &str, blocks: &[Value]) -> Result<Vec<String>> {
        let mut state = self.state.lock().expect("lock state");
        state.append_calls.push((page_id.to_string(), blocks.len()));
        for block in blocks {
            if let Some(text) = extract_paragraph_text(block) {
                state.appended_paragraph_texts.push(text);
            }
        }
        let mut out = Vec::new();
        for _ in blocks {
            let block_id = format!("block_{}", state.next_block);
            state.next_block += 1;
            out.push(block_id);
        }
        Ok(out)
    }

    async fn update_paragraph_block(&self, _block_id: &str, _text: &str) -> Result<()> {
        let mut state = self.state.lock().expect("lock state");
        state.update_block_calls += 1;
        Ok(())
    }
}

fn seed_sync_row(
    conn: &Connection,
    suffix: &str,
    title: &str,
    summary: &str,
    labels_json: &str,
    confidence: f64,
    retry_count: i64,
) {
    std::env::set_var("B2_G2_AI_ENABLED", "0");
    std::env::set_var("HELPER_DISABLE_SHELL_ENV_FALLBACK", "1");
    std::env::set_var("NOTION_TREE_FIXED_FIRST_LEVEL", "0");
    let source_id = format!("src_{suffix}");
    let normalized_id = format!("norm_{suffix}");
    let analysis_id = format!("ana_{suffix}");
    let sync_id = format!("sync_{suffix}");
    let now = "2026-02-25 21:30:00";
    conn.execute(
        "INSERT INTO source_items(
            id, source, external_id, source_url, url_hash, title, content_raw, published_at, collected_at, status
         ) VALUES (?1,'wechat',?2,?3,?4,?5,?6,?7,?8,'new')",
        params![
            source_id,
            format!("ext_{suffix}"),
            format!("https://example.com/{suffix}"),
            format!("h_{suffix}"),
            title,
            "body",
            now,
            now
        ],
    )
    .expect("insert source_items");
    conn.execute(
        "INSERT INTO normalized_items(
            id, source_item_id, canonical_url, canonical_url_hash, text_clean, fingerprint, quality_score, value_score, gate_status, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,'body',?5,80,0.8,'direct',?6,?6)",
        params![
            normalized_id,
            source_id,
            format!("https://example.com/{suffix}"),
            format!("uh_{suffix}"),
            format!("fp_{suffix}"),
            now
        ],
    )
    .expect("insert normalized_items");
    conn.execute(
        "INSERT INTO analysis_results(
            id, normalized_item_id, classification_json, summary_text, c_score, s_score, j_score, d_score, value_score, quality_score, review_required, review_status, final_status, created_at, updated_at
         ) VALUES (?1,?2,?3,?4,?5,0.9,0.8,0.8,0.85,82,0,'skipped','direct',?6,?6)",
        params![analysis_id, normalized_id, labels_json, summary, confidence, now],
    )
    .expect("insert analysis_results");
    conn.execute(
        "INSERT INTO sync_records(
            id, normalized_item_id, target, sync_state, retry_count, next_retry_at, created_at, updated_at
         ) VALUES (?1,?2,'notion','pending',?3,NULL,?4,?4)",
        params![sync_id, normalized_id, retry_count, now],
    )
    .expect("insert sync_records");
}

fn set_app_config(conn: &Connection, key: &str, value: &str) {
    conn.execute(
        "INSERT INTO app_config(key, value, updated_at)
         VALUES (?1, ?2, '2026-02-25 21:30:00')
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
        params![key, value],
    )
    .expect("set app config");
}

#[test]
fn page_tree_first_sync_creates_category_week_item_pages() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "first",
        "Weekly Journal",
        "summary v1",
        "[\"Journal\"]",
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new(None);

    let summary = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("page tree sync");
    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.succeeded, 1);
    assert_eq!(summary.failed, 0);

    let category_count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM notion_tree_nodes WHERE node_type='category'",
            [],
            |r| r.get(0),
        )
        .expect("count category");
    let week_count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM notion_tree_nodes WHERE node_type='week'",
            [],
            |r| r.get(0),
        )
        .expect("count week");
    let item_count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM notion_tree_nodes WHERE node_type='item'",
            [],
            |r| r.get(0),
        )
        .expect("count item");
    assert_eq!(category_count, 1);
    assert_eq!(week_count, 1);
    assert_eq!(item_count, 1);

    let target_record_id: Option<String> = conn
        .query_row(
            "SELECT target_record_id FROM sync_records WHERE id='sync_first'",
            [],
            |r| r.get(0),
        )
        .expect("query target record id");
    assert!(target_record_id.is_some());

    let state = client.snapshot();
    assert_eq!(state.create_calls.len(), 3);
    assert_eq!(state.append_calls.len(), 1);
    assert_eq!(state.append_calls[0].1, 4);
}

#[test]
fn page_tree_repeat_sync_updates_blocks_without_duplicate_item_page() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "repeat",
        "Reading Item",
        "summary v1",
        "[\"Reading List\"]",
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new(None);

    let first = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("first sync");
    assert_eq!(first.succeeded, 1);

    conn.execute(
        "UPDATE analysis_results SET summary_text='summary v2' WHERE normalized_item_id='norm_repeat'",
        [],
    )
    .expect("update summary");
    conn.execute(
        "UPDATE sync_records
         SET sync_state='pending', retry_count=0, next_retry_at=NULL, updated_at='2026-02-25 21:40:00'
         WHERE id='sync_repeat'",
        [],
    )
    .expect("requeue sync row");

    let second = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("second sync");
    assert_eq!(second.succeeded, 1);

    let item_count: i64 = conn
        .query_row(
            "SELECT COUNT(1) FROM notion_tree_nodes WHERE node_type='item' AND normalized_item_id='norm_repeat'",
            [],
            |r| r.get(0),
        )
        .expect("count item");
    assert_eq!(item_count, 1);

    let state = client.snapshot();
    assert_eq!(state.create_calls.len(), 3);
    assert_eq!(state.append_calls.len(), 1);
    assert_eq!(state.append_calls[0].1, 4);
    assert!(state.update_block_calls >= 4);
}

#[test]
fn page_tree_category_migration_moves_item_and_records_history() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "move",
        "Movable Item",
        "summary v1",
        "[\"Journal\"]",
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new(None);

    let first = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("first sync");
    assert_eq!(first.succeeded, 1);

    let old_parent: String = conn
        .query_row(
            "SELECT parent_page_id FROM notion_tree_nodes
             WHERE node_type='item' AND normalized_item_id='norm_move'",
            [],
            |r| r.get(0),
        )
        .expect("query old parent");

    conn.execute(
        "UPDATE analysis_results SET classification_json='[\"Reading List\"]' WHERE normalized_item_id='norm_move'",
        [],
    )
    .expect("update labels");
    conn.execute(
        "UPDATE sync_records
         SET sync_state='pending', retry_count=0, next_retry_at=NULL, updated_at='2026-02-25 21:45:00'
         WHERE id='sync_move'",
        [],
    )
    .expect("requeue sync");

    let second = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("second sync");
    assert_eq!(second.succeeded, 1);

    let (new_parent, history_json): (String, String) = conn
        .query_row(
            "SELECT parent_page_id, category_history_json FROM notion_tree_nodes
             WHERE node_type='item' AND normalized_item_id='norm_move'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query moved item");
    assert_ne!(new_parent, old_parent);
    assert!(history_json.contains("Journal"));
    assert!(history_json.contains("Reading List"));

    let state = client.snapshot();
    assert_eq!(state.move_calls.len(), 1);
}

#[test]
fn page_tree_failed_item_is_dead_lettered_after_retry_exhausted() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "fail",
        "Fail Item",
        "summary v1",
        "[\"Journal\"]",
        0.9,
        2,
    );
    let client = MockNotionTreeClient::new(Some("Fail Item"));

    let summary = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("sync run");
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.dead_lettered, 1);

    let sync_state: String = conn
        .query_row(
            "SELECT sync_state FROM sync_records WHERE id='sync_fail'",
            [],
            |r| r.get(0),
        )
        .expect("query sync state");
    assert_eq!(sync_state, "failed");

    let dlq_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM dead_letter", [], |r| r.get(0))
        .expect("query dead_letter");
    assert!(dlq_count >= 1);
}

#[test]
fn page_tree_failure_before_exhaustion_marks_retry_and_publish_task_failed() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "retrying",
        "Retry Item",
        "summary v1",
        "[\"Journal\"]",
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new(Some("Retry Item"));

    let summary = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("sync run");
    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.requeued, 1);
    assert_eq!(summary.dead_lettered, 0);

    let (sync_state, retry_count, next_retry_at, error_code): (
        String,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT sync_state, retry_count, next_retry_at, last_error_code
             FROM sync_records
             WHERE id='sync_retrying'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query sync retry state");
    assert_eq!(sync_state, "retry");
    assert_eq!(retry_count, 1);
    assert!(next_retry_at.is_some());
    assert_eq!(error_code.as_deref(), Some("MOD-3002"));

    let (task_state, attempt_count, task_next_retry_at, task_error_code): (
        String,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT state, attempt_count, next_retry_at, error_code
             FROM publish_tasks
             WHERE id='pt_src_retrying'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query publish retry state");
    assert_eq!(task_state, "failed");
    assert_eq!(attempt_count, 1);
    assert!(task_next_retry_at.is_some());
    assert_eq!(task_error_code.as_deref(), Some("MOD-3002"));

    let dlq_count: i64 = conn
        .query_row("SELECT COUNT(1) FROM dead_letter", [], |r| r.get(0))
        .expect("query dead_letter count");
    assert_eq!(dlq_count, 0);
}

#[test]
fn page_tree_rate_limited_failure_maps_mod_3429_and_requeues() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "rl",
        "Rate Limit Item",
        "summary v1",
        "[\"Journal\"]",
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new_with_error(
        Some("Rate Limit Item"),
        "notion create child page non-success 429: rate_limited",
    );

    let summary = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("sync run");
    assert_eq!(summary.scanned, 1);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.requeued, 1);
    assert_eq!(summary.dead_lettered, 0);

    let (sync_state, retry_count, next_retry_at, error_code): (
        String,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT sync_state, retry_count, next_retry_at, last_error_code
             FROM sync_records
             WHERE id='sync_rl'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query sync retry state");
    assert_eq!(sync_state, "retry");
    assert_eq!(retry_count, 1);
    assert!(next_retry_at.is_some());
    assert_eq!(error_code.as_deref(), Some("MOD-3429"));

    let (task_state, attempt_count, task_next_retry_at, task_error_code): (
        String,
        i64,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT state, attempt_count, next_retry_at, error_code
             FROM publish_tasks
             WHERE id='pt_src_rl'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("query publish retry state");
    assert_eq!(task_state, "failed");
    assert_eq!(attempt_count, 1);
    assert!(task_next_retry_at.is_some());
    assert_eq!(task_error_code.as_deref(), Some("MOD-3429"));
}

#[test]
fn page_tree_skips_future_retry_items() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "future",
        "Future Item",
        "summary v1",
        "[\"Journal\"]",
        0.9,
        1,
    );
    conn.execute(
        "UPDATE sync_records
         SET sync_state='retry',
             retry_count=1,
             next_retry_at='2099-01-01 00:00:00',
             updated_at='2026-02-25 21:40:00'
         WHERE id='sync_future'",
        [],
    )
    .expect("set future retry");
    let client = MockNotionTreeClient::new(None);

    let summary = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("sync run");
    assert_eq!(summary.scanned, 0);
    assert_eq!(summary.attempted, 0);
    assert_eq!(summary.succeeded, 0);
    assert_eq!(summary.failed, 0);

    let (task_state, next_retry_at): (String, Option<String>) = conn
        .query_row(
            "SELECT state, next_retry_at
             FROM publish_tasks
             WHERE id='pt_src_future'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("query future publish state");
    assert_eq!(task_state, "failed");
    assert_eq!(next_retry_at.as_deref(), Some("2099-01-01 00:00:00"));
}

#[test]
fn page_tree_growth_limit_zero_routes_custom_to_unknown_with_reason() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    set_app_config(&conn, "notion.tree.category_growth_limit", "0");
    seed_sync_row(
        &conn,
        "limit0",
        "Limit Zero",
        "summary",
        "[\"Custom New\"]",
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new(None);

    let summary = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("sync run");
    assert_eq!(summary.succeeded, 1);

    let category_name: String = conn
        .query_row(
            "SELECT category_name FROM notion_tree_nodes
             WHERE node_type='item' AND normalized_item_id='norm_limit0'",
            [],
            |r| r.get(0),
        )
        .expect("query category");
    assert_eq!(category_name, "Inbox");

    let route_reason: Option<String> = conn
        .query_row(
            "SELECT route_reason FROM notion_tree_nodes
             WHERE node_type='item' AND normalized_item_id='norm_limit0'",
            [],
            |r| r.get(0),
        )
        .expect("query route_reason");
    assert_eq!(route_reason.as_deref(), Some("growth_limit_exceeded"));
}

#[test]
fn page_tree_growth_limit_applies_for_new_custom_categories() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    set_app_config(&conn, "notion.tree.category_growth_limit", "1");
    seed_sync_row(
        &conn,
        "limit1a",
        "Limit A",
        "summary",
        "[\"Custom A\"]",
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new(None);

    let first = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("first sync");
    assert_eq!(first.succeeded, 1);

    let first_category: String = conn
        .query_row(
            "SELECT category_name FROM notion_tree_nodes
             WHERE node_type='item' AND normalized_item_id='norm_limit1a'",
            [],
            |r| r.get(0),
        )
        .expect("query first category");
    assert_eq!(first_category, "Custom A");

    seed_sync_row(
        &conn,
        "limit1b",
        "Limit B",
        "summary",
        "[\"Custom B\"]",
        0.9,
        0,
    );
    let second = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("second sync");
    assert_eq!(second.succeeded, 1);

    let second_category: String = conn
        .query_row(
            "SELECT category_name FROM notion_tree_nodes
             WHERE node_type='item' AND normalized_item_id='norm_limit1b'",
            [],
            |r| r.get(0),
        )
        .expect("query second category");
    assert_eq!(second_category, "Inbox");
}

#[test]
fn page_tree_structured_fields_fallback_writes_stable_template() {
    let conn = Connection::open_in_memory().expect("open memory");
    run_migrations(&conn).expect("migrations");
    seed_sync_row(
        &conn,
        "fallback",
        "Fallback Item",
        "",
        r#"{"labels":["学习"]}"#,
        0.9,
        0,
    );
    let client = MockNotionTreeClient::new(None);

    let summary = block_on(sync_pending_page_tree_with_conn_with_options(
        &conn,
        &client,
        "root_page",
        10,
        0,
    ))
    .expect("sync run");
    assert_eq!(summary.succeeded, 1);

    let category_name: String = conn
        .query_row(
            "SELECT category_name FROM notion_tree_nodes
             WHERE node_type='item' AND normalized_item_id='norm_fallback'",
            [],
            |r| r.get(0),
        )
        .expect("query category");
    assert_eq!(category_name, "学习");

    let state = client.snapshot();
    assert_eq!(state.append_calls.len(), 1);
    assert_eq!(state.append_calls[0].1, 4);
    assert!(state
        .appended_paragraph_texts
        .iter()
        .any(|text| text.starts_with("关键要点")));
    assert!(state
        .appended_paragraph_texts
        .iter()
        .any(|text| text.contains("暂无要点")));
}
