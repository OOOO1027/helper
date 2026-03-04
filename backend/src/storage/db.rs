use rusqlite::{Connection, OptionalExtension};

use crate::Result;

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub path: String,
    pub key: String,
}

const MIGRATIONS: &[(&str, &str)] = &[
    ("001_init", include_str!("migrations/001_init.sql")),
    (
        "002_seed_defaults",
        include_str!("migrations/002_seed_defaults.sql"),
    ),
    (
        "003_dead_letter_replay_audit",
        include_str!("migrations/003_dead_letter_replay_audit.sql"),
    ),
    (
        "004_dead_letter_replay_actor_latency",
        include_str!("migrations/004_dead_letter_replay_actor_latency.sql"),
    ),
    (
        "005_sync_daily_stats_view",
        include_str!("migrations/005_sync_daily_stats_view.sql"),
    ),
    (
        "006_notion_page_tree",
        include_str!("migrations/006_notion_page_tree.sql"),
    ),
    (
        "007_publish_domain",
        include_str!("migrations/007_publish_domain.sql"),
    ),
    (
        "008_publish_audit_observability",
        include_str!("migrations/008_publish_audit_observability.sql"),
    ),
    (
        "009_publish_audit_history_filters",
        include_str!("migrations/009_publish_audit_history_filters.sql"),
    ),
    (
        "010_notion_tree_content_enhancements",
        include_str!("migrations/010_notion_tree_content_enhancements.sql"),
    ),
    (
        "011_notion_route_reason_tracking",
        include_str!("migrations/011_notion_route_reason_tracking.sql"),
    ),
];

pub fn open_sqlcipher(cfg: &DbConfig) -> Result<Connection> {
    let conn = Connection::open(&cfg.path)?;
    let escaped_key = cfg.key.replace('\'', "''");
    conn.execute_batch(&format!(
        "
        PRAGMA key = '{escaped_key}';
        PRAGMA cipher_compatibility = 4;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        "
    ))?;
    Ok(conn)
}

pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
          id TEXT PRIMARY KEY,
          applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        ",
    )?;

    for (id, sql) in MIGRATIONS {
        let already_applied: Option<String> = conn
            .query_row(
                "SELECT id FROM schema_migrations WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        if already_applied.is_none() {
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO schema_migrations(id) VALUES(?1)", [id])?;
        }
    }
    Ok(())
}
