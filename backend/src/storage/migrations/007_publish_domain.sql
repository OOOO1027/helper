ALTER TABLE source_items ADD COLUMN collected_day TEXT;

UPDATE source_items
SET collected_day = substr(collected_at, 1, 10)
WHERE collected_day IS NULL;

CREATE INDEX IF NOT EXISTS idx_source_items_status_day
ON source_items(status, collected_day, collected_at DESC);

CREATE TABLE IF NOT EXISTS publish_tasks (
  id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'pending'
    CHECK (state IN ('pending', 'processing', 'published', 'failed', 'ignored')),
  attempt_count INTEGER NOT NULL DEFAULT 0,
  next_retry_at TEXT,
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_publish_tasks_state_retry
ON publish_tasks(state, next_retry_at, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_publish_tasks_item
ON publish_tasks(item_id);

CREATE TABLE IF NOT EXISTS notion_page_refs (
  id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  notion_page_id TEXT NOT NULL,
  category TEXT,
  week_key TEXT,
  published_hash TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(item_id)
);

CREATE INDEX IF NOT EXISTS idx_notion_page_refs_item
ON notion_page_refs(item_id);

CREATE INDEX IF NOT EXISTS idx_notion_page_refs_page
ON notion_page_refs(notion_page_id);

CREATE TABLE IF NOT EXISTS source_state (
  item_id TEXT PRIMARY KEY,
  active_state TEXT NOT NULL DEFAULT 'active'
    CHECK (active_state IN ('active', 'inactive')),
  last_seen_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_source_state_active
ON source_state(active_state, updated_at DESC);

CREATE TABLE IF NOT EXISTS publish_audit (
  id TEXT PRIMARY KEY,
  publish_task_id TEXT,
  item_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('success', 'partial', 'failed', 'skipped')),
  latency_ms INTEGER,
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_publish_audit_created
ON publish_audit(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_publish_audit_item
ON publish_audit(item_id, created_at DESC);
