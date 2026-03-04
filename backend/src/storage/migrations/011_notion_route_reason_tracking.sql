ALTER TABLE notion_tree_nodes ADD COLUMN route_reason TEXT;

ALTER TABLE notion_page_refs ADD COLUMN route_reason TEXT;

CREATE INDEX IF NOT EXISTS idx_notion_page_refs_route_reason
ON notion_page_refs(route_reason, updated_at DESC);
