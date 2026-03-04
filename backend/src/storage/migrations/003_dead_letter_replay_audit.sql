ALTER TABLE dead_letter ADD COLUMN replay_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE dead_letter ADD COLUMN last_replayed_at TEXT;
ALTER TABLE dead_letter ADD COLUMN last_replay_status TEXT;

