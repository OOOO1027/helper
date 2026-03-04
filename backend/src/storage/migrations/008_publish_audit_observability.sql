ALTER TABLE publish_audit ADD COLUMN pipeline_stage TEXT NOT NULL DEFAULT 'publish_approved_to_notion';
ALTER TABLE publish_audit ADD COLUMN sync_mode TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE publish_audit ADD COLUMN retryable INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_publish_audit_stage_created
ON publish_audit(pipeline_stage, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_publish_audit_retryable_created
ON publish_audit(retryable, created_at DESC);
