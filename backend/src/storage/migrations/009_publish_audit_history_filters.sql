CREATE INDEX IF NOT EXISTS idx_publish_audit_history_filters
ON publish_audit(created_at DESC, sync_mode, retryable, status, request_id);
