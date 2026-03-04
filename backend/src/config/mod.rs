//! Centralised configuration reading utilities.
//!
//! Priority chain (highest → lowest):
//!   1. Shell-bootstrapped env var  (via `env_with_shell_fallback`)
//!   2. `app_config` database row
//!   3. Hard-coded default
//!
//! All functions that need a DB connection take `&rusqlite::Connection`
//! (required) or `Option<&rusqlite::Connection>` (optional/best-effort).

use rusqlite::{Connection, OptionalExtension};

use crate::env_runtime::env_with_shell_fallback;
use crate::{BackendError, Result};

// ── Raw DB readers ────────────────────────────────────────────────────────────

pub fn read_app_config_string(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_config WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(BackendError::Storage)
}

pub fn read_app_config_f64(conn: &Connection, key: &str) -> Result<Option<f64>> {
    Ok(read_app_config_string(conn, key)?.and_then(|v| v.parse::<f64>().ok()))
}

pub fn read_app_config_i64(conn: &Connection, key: &str) -> Result<Option<i64>> {
    Ok(read_app_config_string(conn, key)?.and_then(|v| v.parse::<i64>().ok()))
}

// ── Env + required DB resolvers ───────────────────────────────────────────────

/// Env override → DB fallback.  Returns `None` if neither is set.
pub fn env_or_db_string(
    conn: &Connection,
    env_key: &str,
    app_config_key: &str,
) -> Result<Option<String>> {
    if let Some(value) = env_with_shell_fallback(env_key) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed));
        }
    }
    read_app_config_string(conn, app_config_key)
}

/// Env override → DB fallback.  Returns `None` if neither is set.
/// Env value must parse as `f64` or a validation error is returned.
pub fn env_or_db_f64(
    conn: &Connection,
    env_key: &str,
    app_config_key: &str,
) -> Result<Option<f64>> {
    if let Some(value) = env_with_shell_fallback(env_key) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            let parsed = trimmed.parse::<f64>().map_err(|_| {
                BackendError::Validation(format!("{env_key} must be a valid float"))
            })?;
            return Ok(Some(parsed));
        }
    }
    read_app_config_f64(conn, app_config_key)
}

/// Env override → DB fallback.  Returns `None` if neither is set.
/// Env value must parse as `i64` or a validation error is returned.
pub fn env_or_db_i64(
    conn: &Connection,
    env_key: &str,
    app_config_key: &str,
) -> Result<Option<i64>> {
    if let Some(value) = env_with_shell_fallback(env_key) {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            let parsed = trimmed.parse::<i64>().map_err(|_| {
                BackendError::Validation(format!("{env_key} must be a valid integer"))
            })?;
            return Ok(Some(parsed));
        }
    }
    read_app_config_i64(conn, app_config_key)
}

// ── Higher-level resolvers (optional conn) ────────────────────────────────────

/// Env override → optional DB fallback → hard-coded default string.
pub fn resolve_string_config(
    conn: Option<&Connection>,
    env_key: &str,
    app_config_key: &str,
    default: &str,
) -> Result<String> {
    if let Some(value) = env_with_shell_fallback(env_key) {
        let normalized = value.trim().to_string();
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }
    if let Some(db) = conn {
        if let Some(value) = read_app_config_string(db, app_config_key)? {
            let normalized = value.trim().to_string();
            if !normalized.is_empty() {
                return Ok(normalized);
            }
        }
    }
    Ok(default.to_string())
}

/// Env override → optional DB fallback → hard-coded `i64` default.
pub fn resolve_i64_config(
    conn: Option<&Connection>,
    env_key: &str,
    app_config_key: &str,
    default: i64,
) -> Result<i64> {
    if let Some(value) = env_with_shell_fallback(env_key) {
        let normalized = value.trim().to_string();
        if !normalized.is_empty() {
            return normalized.parse::<i64>().map_err(|_| {
                BackendError::Validation(format!("{env_key} must be a valid integer"))
            });
        }
    }
    if let Some(db) = conn {
        if let Some(value) = read_app_config_i64(db, app_config_key)? {
            return Ok(value);
        }
    }
    Ok(default)
}

/// Read the Notion sync mode (`page_tree` | `database`).
///
/// Priority: `NOTION_SYNC_MODE` env → `notion.sync.mode` DB key → `"page_tree"`.
pub fn read_notion_sync_mode(conn: Option<&Connection>) -> Result<String> {
    if let Some(mode) = env_with_shell_fallback("NOTION_SYNC_MODE") {
        let normalized = mode.trim().to_lowercase();
        if !normalized.is_empty() {
            return Ok(normalized);
        }
    }
    if let Some(db) = conn {
        if let Some(mode) = read_app_config_string(db, "notion.sync.mode")? {
            let normalized = mode.trim().to_lowercase();
            if !normalized.is_empty() {
                return Ok(normalized);
            }
        }
    }
    Ok("page_tree".to_string())
}

// ── Env-only helpers ──────────────────────────────────────────────────────────

/// Returns `true` if the env var is set to a non-empty value.
pub fn env_is_configured(key: &str) -> bool {
    env_with_shell_fallback(key)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

/// Read a boolean from an env var (`1 | true | yes | on` → `true`).
pub fn env_bool(key: &str, default_value: bool) -> bool {
    env_with_shell_fallback(key)
        .map(|v| {
            let normalized = v.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default_value)
}

/// Read a `usize` from an env var, falling back to `default_value`.
pub fn env_usize(key: &str, default_value: usize) -> usize {
    env_with_shell_fallback(key)
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(default_value)
}

/// Read a `u64` from an env var, falling back to `default_value`.
pub fn env_u64(key: &str, default_value: u64) -> u64 {
    env_with_shell_fallback(key)
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default_value)
}

/// Read an `f64` from an env var, falling back to `default_value`.
pub fn env_f64(key: &str, default_value: f64) -> f64 {
    env_with_shell_fallback(key)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(default_value)
}

/// Return the value of a required env var as a `String`.
///
/// `NOTION_TOKEN` is always read directly from the process environment
/// (it must not come from a shell-injected fallback for security reasons).
pub fn required_env(key: &str) -> Result<String> {
    if key == "NOTION_TOKEN" {
        let value = std::env::var(key)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                BackendError::Validation(
                    "NOTION_TOKEN is required in current process env".to_string(),
                )
            })?;
        return Ok(value);
    }
    env_with_shell_fallback(key)
        .ok_or_else(|| BackendError::Validation(format!("{key} is required")))
}
