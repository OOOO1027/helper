ALTER TABLE source_items ADD COLUMN cover_url TEXT;
ALTER TABLE source_items ADD COLUMN image_urls_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE notion_tree_nodes ADD COLUMN key_points_block_id TEXT;
ALTER TABLE notion_tree_nodes ADD COLUMN source_link_block_id TEXT;
ALTER TABLE notion_tree_nodes ADD COLUMN tags_block_id TEXT;
ALTER TABLE notion_tree_nodes ADD COLUMN images_block_id TEXT;

UPDATE app_config
SET value='学习|工作|生活|健康|财务|灵感|Inbox',
    updated_at=datetime('now')
WHERE key='notion.tree.default_categories'
  AND (
    value='Journal|Travel Planner|Habit Tracker|Reading List|Inbox'
    OR TRIM(value)=''
  );

UPDATE app_config
SET value='32',
    updated_at=datetime('now')
WHERE key='notion.tree.category_growth_limit'
  AND (TRIM(value)='' OR CAST(value AS INTEGER) <= 0);
