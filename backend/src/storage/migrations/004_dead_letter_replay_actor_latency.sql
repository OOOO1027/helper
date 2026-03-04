ALTER TABLE dead_letter ADD COLUMN last_replayed_by TEXT;
ALTER TABLE dead_letter ADD COLUMN last_replay_latency_ms INTEGER;

