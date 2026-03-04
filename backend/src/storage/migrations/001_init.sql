CREATE TABLE IF NOT EXISTS source_items (
  id TEXT PRIMARY KEY,
  source TEXT NOT NULL CHECK (source IN ('xhs', 'wechat', 'bili')),
  external_id TEXT NOT NULL,
  source_url TEXT,
  url_hash TEXT NOT NULL,
  title TEXT,
  content_raw TEXT NOT NULL,
  published_at TEXT,
  collected_at TEXT NOT NULL DEFAULT (datetime('now')),
  ingest_batch_id TEXT,
  status TEXT NOT NULL DEFAULT 'new',
  UNIQUE(source, external_id)
);

CREATE INDEX IF NOT EXISTS idx_source_items_source_collected
ON source_items(source, collected_at DESC);

CREATE INDEX IF NOT EXISTS idx_source_items_url_hash
ON source_items(url_hash);

CREATE TABLE IF NOT EXISTS normalized_items (
  id TEXT PRIMARY KEY,
  source_item_id TEXT NOT NULL UNIQUE,
  canonical_url TEXT,
  canonical_url_hash TEXT,
  text_clean TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  quality_score REAL NOT NULL DEFAULT 0,
  value_score REAL NOT NULL DEFAULT 0,
  gate_status TEXT NOT NULL CHECK (gate_status IN ('direct', 'review', 'exception')),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(source_item_id) REFERENCES source_items(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_normalized_items_gate_status
ON normalized_items(gate_status, created_at DESC);

CREATE TABLE IF NOT EXISTS dedupe_index (
  id TEXT PRIMARY KEY,
  normalized_item_id TEXT NOT NULL,
  url_hash TEXT,
  simhash TEXT,
  embedding_ref TEXT,
  rule_type TEXT NOT NULL CHECK (rule_type IN ('hard_url', 'semantic')),
  confidence REAL NOT NULL DEFAULT 0,
  duplicate_of TEXT,
  first_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
  last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(normalized_item_id) REFERENCES normalized_items(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_dedupe_index_url_hash
ON dedupe_index(url_hash);

CREATE TABLE IF NOT EXISTS analysis_results (
  id TEXT PRIMARY KEY,
  normalized_item_id TEXT NOT NULL UNIQUE,
  primary_model TEXT NOT NULL DEFAULT 'qwen-plus',
  review_model TEXT DEFAULT 'gpt-5-mini',
  classification_json TEXT,
  summary_text TEXT,
  c_score REAL NOT NULL DEFAULT 0,
  s_score REAL NOT NULL DEFAULT 0,
  j_score REAL NOT NULL DEFAULT 0,
  d_score REAL NOT NULL DEFAULT 0,
  value_score REAL NOT NULL DEFAULT 0,
  quality_score REAL NOT NULL DEFAULT 0,
  review_required INTEGER NOT NULL CHECK (review_required IN (0,1)),
  review_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (review_status IN ('pending', 'pass', 'reject', 'skipped')),
  final_status TEXT NOT NULL DEFAULT 'review'
    CHECK (final_status IN ('direct', 'review', 'exception')),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(normalized_item_id) REFERENCES normalized_items(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS review_queue (
  id TEXT PRIMARY KEY,
  normalized_item_id TEXT NOT NULL,
  analysis_result_id TEXT NOT NULL,
  priority REAL NOT NULL,
  reason TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'pending'
    CHECK (state IN ('pending', 'processing', 'done', 'rejected')),
  assigned_to TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(normalized_item_id) REFERENCES normalized_items(id) ON DELETE CASCADE,
  FOREIGN KEY(analysis_result_id) REFERENCES analysis_results(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_review_queue_state_priority
ON review_queue(state, priority DESC, created_at ASC);

CREATE TABLE IF NOT EXISTS sync_records (
  id TEXT PRIMARY KEY,
  normalized_item_id TEXT NOT NULL,
  target TEXT NOT NULL DEFAULT 'notion',
  target_record_id TEXT,
  sync_state TEXT NOT NULL DEFAULT 'pending'
    CHECK (sync_state IN ('pending', 'success', 'failed', 'retry')),
  retry_count INTEGER NOT NULL DEFAULT 0,
  next_retry_at TEXT,
  last_error_code TEXT,
  last_error_message TEXT,
  last_synced_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(normalized_item_id) REFERENCES normalized_items(id) ON DELETE CASCADE,
  UNIQUE(target, normalized_item_id)
);

CREATE TABLE IF NOT EXISTS job_runs (
  id TEXT PRIMARY KEY,
  job_type TEXT NOT NULL,
  source TEXT,
  status TEXT NOT NULL CHECK (status IN ('started', 'success', 'failed', 'partial', 'skipped')),
  scheduled_at TEXT,
  started_at TEXT,
  finished_at TEXT,
  success_count INTEGER NOT NULL DEFAULT 0,
  fail_count INTEGER NOT NULL DEFAULT 0,
  metadata_json TEXT,
  error_code TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_job_runs_job_type_created
ON job_runs(job_type, created_at DESC);

CREATE TABLE IF NOT EXISTS budget_ledger (
  id TEXT PRIMARY KEY,
  day TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  purpose TEXT NOT NULL,
  tokens_in INTEGER NOT NULL DEFAULT 0,
  tokens_out INTEGER NOT NULL DEFAULT 0,
  cost_cny REAL NOT NULL DEFAULT 0,
  job_run_id TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY(job_run_id) REFERENCES job_runs(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_budget_ledger_day
ON budget_ledger(day, provider, model);

CREATE TABLE IF NOT EXISTS dead_letter (
  id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  stage TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  error_code TEXT NOT NULL,
  error_message TEXT,
  attempt_count INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'resolved')),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_dead_letter_state_created
ON dead_letter(state, created_at DESC);

CREATE TABLE IF NOT EXISTS app_config (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

