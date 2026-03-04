use std::process::Command;

const SHELL_CANDIDATES: &[(&str, &str)] = &[
    // login + interactive, so zsh/bash load user profiles.
    ("zsh", "-lic"),
    ("bash", "-lc"),
    ("sh", "-lc"),
];

fn valid_env_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

pub fn env_with_shell_fallback(key: &str) -> Option<String> {
    if !valid_env_key(key) {
        return None;
    }
    if let Ok(value) = std::env::var(key) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    if std::env::var("HELPER_DISABLE_SHELL_ENV_FALLBACK")
        .ok()
        .as_deref()
        == Some("1")
    {
        return None;
    }
    for (shell, shell_flag) in SHELL_CANDIDATES {
        let command = format!("printenv {key}");
        let output = Command::new(shell).args([*shell_flag, &command]).output();
        let Ok(out) = output else {
            continue;
        };
        if !out.status.success() {
            continue;
        }
        let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

pub fn bootstrap_env_from_shell(keys: &[&str]) {
    for key in keys {
        if std::env::var(key)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .is_some()
        {
            continue;
        }
        if let Some(value) = env_with_shell_fallback(key) {
            std::env::set_var(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bootstrap_env_from_shell, env_with_shell_fallback, valid_env_key};

    #[test]
    fn valid_env_key_rejects_invalid_chars() {
        assert!(valid_env_key("HELPER_DB_PATH"));
        assert!(!valid_env_key("helper_db_path"));
        assert!(!valid_env_key("A-B"));
    }

    #[test]
    fn bootstrap_keeps_existing_value() {
        std::env::set_var("HELPER_TEST_KEEP", "v1");
        bootstrap_env_from_shell(&["HELPER_TEST_KEEP"]);
        assert_eq!(
            std::env::var("HELPER_TEST_KEEP").expect("existing"),
            "v1".to_string()
        );
        std::env::remove_var("HELPER_TEST_KEEP");
    }

    #[test]
    fn env_with_shell_fallback_returns_none_for_invalid_key() {
        assert!(env_with_shell_fallback("INVALID-KEY").is_none());
    }
}
