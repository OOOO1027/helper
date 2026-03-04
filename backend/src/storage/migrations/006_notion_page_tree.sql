CREATE TABLE IF NOT EXISTS notion_tree_nodes (
  id TEXT PRIMARY KEY,
  node_type TEXT NOT NULL CHECK (node_type IN ('category', 'week', 'item')),
  node_key TEXT NOT NULL,
  page_id TEXT NOT NULL,
  parent_page_id TEXT NOT NULL,
  title TEXT NOT NULL,
  normalized_item_id TEXT,
  category_name TEXT,
  week_key TEXT,
  meta_block_id TEXT,
  summary_block_id TEXT,
  category_history_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_notion_tree_nodes_scope
ON notion_tree_nodes(node_type, parent_page_id, node_key);

CREATE UNIQUE INDEX IF NOT EXISTS uq_notion_tree_nodes_item
ON notion_tree_nodes(normalized_item_id)
WHERE normalized_item_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_notion_tree_nodes_parent_type
ON notion_tree_nodes(node_type, parent_page_id);
