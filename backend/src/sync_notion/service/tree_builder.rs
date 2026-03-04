use chrono::{Local, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::sync_notion::notion_api::NotionTreeClient;
use crate::sync_notion::tree::{
    clean_item_title, normalize_key, route_category_with_growth_limit, week_key_and_title,
};
use crate::{BackendError, Result};
use super::{
    stable_hash, parse_published_at, render_status_metadata,
    NotionTreeConfig, PendingSyncRow, StructuredContentState, TreeItemNode,
};
use super::render::{
    normalize_node_key, paragraph_block, render_images_block, render_key_points_block,
    render_source_link_block, render_summary_block, render_tags_block,
};
use super::ai_content::{build_structured_content, refresh_budget_guard_state};
use super::state_persistence::persist_structured_snapshot;

pub(super) async fn load_known_custom_categories<C: NotionTreeClient>(
    conn: &Connection,
    client: &C,
    root_page_id: &str,
    config: &NotionTreeConfig,
) -> Result<HashSet<String>> {
    let default_set = config
        .default_categories
        .iter()
        .map(|v| normalize_node_key(v))
        .collect::<HashSet<_>>();
    let unknown_key = normalize_node_key(&config.unknown_category);
    let mut known_custom = HashSet::new();

    let mut stmt = conn.prepare(
        "SELECT node_key FROM notion_tree_nodes
         WHERE node_type='category' AND parent_page_id=?1",
    )?;
    let rows = stmt.query_map([root_page_id], |row| row.get::<_, String>(0))?;
    for row in rows {
        let key = normalize_node_key(&row?);
        if key != unknown_key && !default_set.contains(&key) {
            known_custom.insert(key);
        }
    }

    let root_pages = client.list_child_pages(root_page_id).await?;
    for page in root_pages {
        let key = normalize_key(&page.title);
        if key != unknown_key && !default_set.contains(&key) {
            known_custom.insert(key);
        }
    }
    Ok(known_custom)
}

pub(super) async fn upsert_tree_item<C: NotionTreeClient>(
    conn: &Connection,
    client: &C,
    root_page_id: &str,
    config: &NotionTreeConfig,
    row: &PendingSyncRow,
    known_custom_categories: &mut HashSet<String>,
    content_state: &mut StructuredContentState,
) -> Result<String> {
    let mut route_labels = Vec::new();
    if let Some(final_category) = row
        .final_category
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
    {
        route_labels.push(final_category.to_string());
    }
    for label in row.labels.iter().chain(row.tags.iter()) {
        let trimmed = label.trim();
        if !trimmed.is_empty() && !route_labels.iter().any(|v| v == trimmed) {
            route_labels.push(trimmed.to_string());
        }
    }
    if route_labels.is_empty() {
        route_labels.push(config.unknown_category.clone());
    }
    let route_decision = route_category_with_growth_limit(
        &route_labels,
        row.confidence,
        config.route_min_confidence,
        &config.unknown_category,
        &config.default_categories,
        known_custom_categories,
        config.category_growth_limit,
    );
    let category = route_decision.category.clone();
    let category_key = normalize_node_key(&category);
    let category_page_id = ensure_group_page(
        conn,
        client,
        "category",
        root_page_id,
        &category_key,
        &category,
        None,
        None,
    )
    .await?;

    if route_decision.created_new_custom_category {
        known_custom_categories.insert(category_key.clone());
    }

    let ts = parse_published_at(&row.published_at).unwrap_or_else(Utc::now);
    let (week_key, week_title) =
        week_key_and_title(ts, &config.timezone).map_err(BackendError::Validation)?;
    let week_page_id = ensure_group_page(
        conn,
        client,
        "week",
        &category_page_id,
        &week_key,
        &week_title,
        Some(&category),
        Some(&week_key),
    )
    .await?;

    let item_title = clean_item_title(&row.title);
    let route_reason = route_decision
        .reason
        .as_deref()
        .or(row.route_reason.as_deref());
    refresh_budget_guard_state(conn, content_state)?;
    let content = build_structured_content(row, route_reason, content_state).await?;
    if content.quality_state == "failed" {
        return Err(BackendError::Validation(format!(
            "content quality gate failed: score={} source={}",
            content.quality_score.round(),
            content.content_source
        )));
    }
    let summary_block_text = render_summary_block(&content.summary);
    let key_points_block_text = render_key_points_block(&content.key_points, &content.summary);
    let source_link_block_text = render_source_link_block(row.source_url.as_deref());
    let tags_block_text = render_tags_block(&content.tags, &row.labels);
    let metadata = render_status_metadata(row, &category, route_reason, &content);
    let images_block_text = render_images_block(row.cover_url.as_deref(), &row.image_urls);
    persist_structured_snapshot(conn, row, &content, &category, route_reason)?;
    let existing = get_tree_item_node(conn, &row.normalized_item_id)?;

    let (
        item_id,
        item_page_id,
        final_meta_block_id,
        final_summary_block_id,
        final_key_points_block_id,
        final_source_link_block_id,
        final_tags_block_id,
        final_images_block_id,
        history_json,
    ) = if let Some(existing_item) = existing {
        if existing_item.parent_page_id != week_page_id {
            client
                .move_page(&existing_item.page_id, &week_page_id)
                .await
                .map_err(|e| BackendError::Internal(format!("move notion page failed: {e}")))?;
        }
        client
            .update_page_title(&existing_item.page_id, &item_title)
            .await
            .map_err(|e| BackendError::Internal(format!("update notion page title failed: {e}")))?;

        let mut meta_block_id = existing_item.meta_block_id.clone();
        let mut summary_block_id = existing_item.summary_block_id.clone();
        let mut key_points_block_id = existing_item.key_points_block_id.clone();
        let mut source_link_block_id = existing_item.source_link_block_id.clone();
        let mut tags_block_id = existing_item.tags_block_id.clone();
        let mut images_block_id = existing_item.images_block_id.clone();
        if let Some(ref block_id) = meta_block_id {
            if metadata.trim().is_empty() {
                client
                    .update_paragraph_block(block_id, " ")
                    .await
                    .map_err(|e| {
                        BackendError::Internal(format!("clear metadata block failed: {e}"))
                    })?;
            } else {
                client
                    .update_paragraph_block(block_id, &metadata)
                    .await
                    .map_err(|e| {
                        BackendError::Internal(format!("update metadata block failed: {e}"))
                    })?;
            }
        }
        if let Some(ref block_id) = summary_block_id {
            client
                .update_paragraph_block(block_id, &summary_block_text)
                .await
                .map_err(|e| BackendError::Internal(format!("update summary block failed: {e}")))?;
        }
        if let Some(ref block_id) = key_points_block_id {
            client
                .update_paragraph_block(block_id, &key_points_block_text)
                .await
                .map_err(|e| {
                    BackendError::Internal(format!("update key_points block failed: {e}"))
                })?;
        }
        if let Some(ref block_id) = source_link_block_id {
            client
                .update_paragraph_block(block_id, &source_link_block_text)
                .await
                .map_err(|e| {
                    BackendError::Internal(format!("update source_link block failed: {e}"))
                })?;
        }
        if let Some(ref block_id) = tags_block_id {
            client
                .update_paragraph_block(block_id, &tags_block_text)
                .await
                .map_err(|e| BackendError::Internal(format!("update tags block failed: {e}")))?;
        }
        if let Some(ref block_id) = images_block_id {
            if images_block_text.trim().is_empty() {
                client
                    .update_paragraph_block(block_id, " ")
                    .await
                    .map_err(|e| {
                        BackendError::Internal(format!("clear images block failed: {e}"))
                    })?;
            } else {
                client
                    .update_paragraph_block(block_id, &images_block_text)
                    .await
                    .map_err(|e| BackendError::Internal(format!("update images block failed: {e}")))?;
            }
        }

        let mut missing_texts = Vec::new();
        let mut missing_kinds = Vec::new();
        if summary_block_id.is_none() {
            missing_kinds.push("summary");
            missing_texts.push(summary_block_text.clone());
        }
        if key_points_block_id.is_none() {
            missing_kinds.push("key_points");
            missing_texts.push(key_points_block_text.clone());
        }
        if source_link_block_id.is_none() {
            missing_kinds.push("source_link");
            missing_texts.push(source_link_block_text.clone());
        }
        if tags_block_id.is_none() {
            missing_kinds.push("tags");
            missing_texts.push(tags_block_text.clone());
        }
        if meta_block_id.is_none() && !metadata.trim().is_empty() {
            missing_kinds.push("metadata");
            missing_texts.push(metadata.clone());
        }
        if images_block_id.is_none() && !images_block_text.trim().is_empty() {
            missing_kinds.push("images");
            missing_texts.push(images_block_text.clone());
        }
        if !missing_texts.is_empty() {
            let blocks = missing_texts
                .iter()
                .map(|text| paragraph_block(text))
                .collect::<Vec<_>>();
            let new_ids = client
                .append_blocks(&existing_item.page_id, &blocks)
                .await
                .map_err(|e| BackendError::Internal(format!("append blocks failed: {e}")))?;
            for (idx, kind) in missing_kinds.iter().enumerate() {
                let block_id = new_ids.get(idx).cloned();
                match *kind {
                    "summary" => summary_block_id = block_id,
                    "key_points" => key_points_block_id = block_id,
                    "source_link" => source_link_block_id = block_id,
                    "tags" => tags_block_id = block_id,
                    "metadata" => meta_block_id = block_id,
                    "images" => images_block_id = block_id,
                    _ => {}
                }
            }
        }

        let history_json = append_category_history(
            &existing_item.category_history_json,
            existing_item.category_name.as_deref(),
            &category,
        );
        (
            existing_item.id,
            existing_item.page_id,
            meta_block_id,
            summary_block_id,
            key_points_block_id,
            source_link_block_id,
            tags_block_id,
            images_block_id,
            history_json,
        )
    } else {
        let page_id = client
            .create_child_page(&week_page_id, &item_title)
            .await
            .map_err(|e| BackendError::Internal(format!("create notion item page failed: {e}")))?;
        let mut blocks = vec![
            paragraph_block(&summary_block_text),
            paragraph_block(&key_points_block_text),
            paragraph_block(&source_link_block_text),
            paragraph_block(&tags_block_text),
        ];
        let metadata_index = if metadata.trim().is_empty() {
            None
        } else {
            let idx = blocks.len();
            blocks.push(paragraph_block(&metadata));
            Some(idx)
        };
        let images_index = if images_block_text.trim().is_empty() {
            None
        } else {
            let idx = blocks.len();
            blocks.push(paragraph_block(&images_block_text));
            Some(idx)
        };
        let block_ids = client
            .append_blocks(&page_id, &blocks)
            .await
            .map_err(|e| {
                BackendError::Internal(format!("append notion item blocks failed: {e}"))
            })?;
        (
            format!("ntn_item_{}", stable_hash(&row.normalized_item_id)),
            page_id,
            metadata_index.and_then(|idx| block_ids.get(idx).cloned()),
            block_ids.first().cloned(),
            block_ids.get(1).cloned(),
            block_ids.get(2).cloned(),
            block_ids.get(3).cloned(),
            images_index.and_then(|idx| block_ids.get(idx).cloned()),
            "[]".to_string(),
        )
    };

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO notion_tree_nodes(
            id, node_type, node_key, page_id, parent_page_id, title, normalized_item_id,
            category_name, week_key, route_reason, meta_block_id, summary_block_id, key_points_block_id,
            source_link_block_id, tags_block_id, images_block_id, category_history_json, created_at, updated_at
         ) VALUES (?1,'item',?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?17)
         ON CONFLICT(id) DO UPDATE SET
            node_key=excluded.node_key,
            page_id=excluded.page_id,
            parent_page_id=excluded.parent_page_id,
            title=excluded.title,
            category_name=excluded.category_name,
            week_key=excluded.week_key,
            route_reason=excluded.route_reason,
            meta_block_id=excluded.meta_block_id,
            summary_block_id=excluded.summary_block_id,
            key_points_block_id=excluded.key_points_block_id,
            source_link_block_id=excluded.source_link_block_id,
            tags_block_id=excluded.tags_block_id,
            images_block_id=excluded.images_block_id,
            category_history_json=excluded.category_history_json,
            updated_at=excluded.updated_at",
        params![
            item_id,
            normalize_node_key(&row.normalized_item_id),
            item_page_id,
            week_page_id,
            item_title,
            row.normalized_item_id,
            category,
            week_key,
            route_reason,
            final_meta_block_id,
            final_summary_block_id,
            final_key_points_block_id,
            final_source_link_block_id,
            final_tags_block_id,
            final_images_block_id,
            history_json,
            now
        ],
    )?;

    Ok(item_page_id)
}

async fn ensure_group_page<C: NotionTreeClient>(
    conn: &Connection,
    client: &C,
    node_type: &str,
    parent_page_id: &str,
    node_key: &str,
    title: &str,
    category_name: Option<&str>,
    week_key: Option<&str>,
) -> Result<String> {
    if let Some(page_id) = conn
        .query_row(
            "SELECT page_id FROM notion_tree_nodes WHERE node_type=?1 AND parent_page_id=?2 AND node_key=?3",
            params![node_type, parent_page_id, node_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(page_id);
    }

    let children = client.list_child_pages(parent_page_id).await?;
    if let Some(existing) = children
        .into_iter()
        .find(|p| p.title.eq_ignore_ascii_case(title))
    {
        upsert_group_node(
            conn,
            node_type,
            node_key,
            &existing.page_id,
            parent_page_id,
            title,
            category_name,
            week_key,
        )?;
        return Ok(existing.page_id);
    }

    let page_id = client.create_child_page(parent_page_id, title).await?;
    upsert_group_node(
        conn,
        node_type,
        node_key,
        &page_id,
        parent_page_id,
        title,
        category_name,
        week_key,
    )?;
    Ok(page_id)
}

fn upsert_group_node(
    conn: &Connection,
    node_type: &str,
    node_key: &str,
    page_id: &str,
    parent_page_id: &str,
    title: &str,
    category_name: Option<&str>,
    week_key: Option<&str>,
) -> Result<()> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let id = format!(
        "ntn_{}_{}",
        node_type,
        stable_hash(&format!("{node_type}|{parent_page_id}|{node_key}"))
    );
    conn.execute(
        "INSERT INTO notion_tree_nodes(
            id, node_type, node_key, page_id, parent_page_id, title, normalized_item_id,
            category_name, week_key, meta_block_id, summary_block_id, category_history_json, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8, NULL, NULL, '[]', ?9, ?9)
         ON CONFLICT(node_type, parent_page_id, node_key) DO UPDATE SET
            page_id=excluded.page_id,
            title=excluded.title,
            category_name=COALESCE(excluded.category_name, notion_tree_nodes.category_name),
            week_key=COALESCE(excluded.week_key, notion_tree_nodes.week_key),
            updated_at=excluded.updated_at",
        params![
            id,
            node_type,
            node_key,
            page_id,
            parent_page_id,
            title,
            category_name,
            week_key,
            now
        ],
    )?;
    Ok(())
}

fn get_tree_item_node(conn: &Connection, normalized_item_id: &str) -> Result<Option<TreeItemNode>> {
    conn.query_row(
        "SELECT
            id,
            page_id,
            parent_page_id,
            category_name,
            category_history_json,
            meta_block_id,
            summary_block_id,
            key_points_block_id,
            source_link_block_id,
            tags_block_id,
            images_block_id
         FROM notion_tree_nodes
         WHERE node_type='item' AND normalized_item_id=?1",
        params![normalized_item_id],
        |row| {
            Ok(TreeItemNode {
                id: row.get(0)?,
                page_id: row.get(1)?,
                parent_page_id: row.get(2)?,
                category_name: row.get(3)?,
                category_history_json: row.get(4)?,
                meta_block_id: row.get(5)?,
                summary_block_id: row.get(6)?,
                key_points_block_id: row.get(7)?,
                source_link_block_id: row.get(8)?,
                tags_block_id: row.get(9)?,
                images_block_id: row.get(10)?,
            })
        },
    )
    .optional()
    .map_err(BackendError::Storage)
}

fn append_category_history(history_json: &str, old: Option<&str>, new: &str) -> String {
    let mut history = serde_json::from_str::<Vec<Value>>(history_json).unwrap_or_default();
    if let Some(old_category) = old {
        if !old_category.eq_ignore_ascii_case(new) {
            history.push(json!({
                "from": old_category,
                "to": new,
                "at": Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
            }));
        }
    }
    serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string())
}
