use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

use crate::pipeline::normalize::{canonicalize_url, extract_urls, normalize_ts};
use crate::{BackendError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WechatImportSummary {
    pub batch_id: String,
    pub total: u32,
    pub imported: u32,
    pub duplicates: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WechatImportedItem {
    pub path: String,
    pub content_raw: String,
    pub source_url: Option<String>,
    pub extracted_urls: Vec<String>,
    pub published_at: Option<String>,
    pub dedupe_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WechatImportError {
    pub path: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WechatImportParsed {
    pub items: Vec<WechatImportedItem>,
    pub duplicates: u32,
    pub errors: Vec<WechatImportError>,
}

pub fn import_wechat_files(paths: &[String]) -> Result<WechatImportSummary> {
    if paths.is_empty() {
        return Err(BackendError::Validation(
            "wechat import paths must not be empty".to_string(),
        ));
    }

    let parsed = parse_wechat_files(paths);
    let total = paths.len() as u32;
    Ok(WechatImportSummary {
        batch_id: Uuid::new_v4().to_string(),
        total,
        imported: parsed.items.len() as u32,
        duplicates: parsed.duplicates,
        failed: parsed.errors.len() as u32,
    })
}

pub fn parse_wechat_files(paths: &[String]) -> WechatImportParsed {
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut seen = HashSet::new();
    let mut duplicates = 0u32;

    for path_str in paths {
        let path = Path::new(path_str);
        match parse_one(path) {
            Ok(item) => {
                if seen.insert(item.dedupe_key.clone()) {
                    items.push(item);
                } else {
                    duplicates += 1;
                }
            }
            Err(e) => errors.push(WechatImportError {
                path: path_str.clone(),
                code: map_error_code(&e).to_string(),
                message: e.to_string(),
                retryable: false,
            }),
        }
    }

    WechatImportParsed {
        items,
        duplicates,
        errors,
    }
}

fn parse_one(path: &Path) -> Result<WechatImportedItem> {
    if !path.exists() {
        return Err(BackendError::Validation(format!(
            "ING-1001 file not found: {}",
            path.display()
        )));
    }
    let ext = path
        .extension()
        .map(|v| v.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let content_raw = match ext.as_str() {
        "txt" | "md" => fs::read_to_string(path).map_err(|e| {
            BackendError::Validation(format!("ING-1001 read failed {}: {e}", path.display()))
        })?,
        "html" | "htm" => {
            let html = fs::read_to_string(path).map_err(|e| {
                BackendError::Validation(format!("ING-1001 read failed {}: {e}", path.display()))
            })?;
            strip_html(&html)
        }
        "pdf" | "doc" | "docx" => extract_with_local_tools(path, &ext)?,
        _ => {
            return Err(BackendError::Validation(format!(
                "PAR-2001 unsupported extension '{}': {}",
                ext,
                path.display()
            )))
        }
    };
    let extracted_urls = extract_urls(&content_raw);
    let source_url = extracted_urls.first().map(|v| canonicalize_url(v));
    let published_at = detect_published_at(&content_raw).or_else(|| file_modified_at(path));
    let dedupe_raw = format!(
        "{}|{}|{}",
        source_url.clone().unwrap_or_default(),
        short_hash(content_raw.as_bytes()),
        published_at.clone().unwrap_or_default()
    );
    let dedupe_key = format!("{:x}", short_hash(dedupe_raw.as_bytes()));

    Ok(WechatImportedItem {
        path: path.display().to_string(),
        content_raw,
        source_url,
        extracted_urls,
        published_at,
        dedupe_key,
    })
}

fn extract_with_local_tools(path: &Path, ext: &str) -> Result<String> {
    match ext {
        "doc" | "docx" => {
            let path_str = path.to_string_lossy().to_string();
            let output = Command::new("textutil")
                .args(["-convert", "txt", "-stdout", &path_str])
                .output();
            match output {
                Ok(out) if out.status.success() => {
                    let txt = String::from_utf8_lossy(&out.stdout).to_string();
                    let cleaned = txt.split_whitespace().collect::<Vec<_>>().join(" ");
                    if cleaned.is_empty() {
                        return Err(BackendError::Validation(format!(
                            "PAR-2001 empty extraction for {}",
                            path.display()
                        )));
                    }
                    Ok(cleaned)
                }
                _ => Err(BackendError::Validation(format!(
                    "PAR-2001 local extraction failed for {}",
                    path.display()
                ))),
            }
        }
        "pdf" => {
            let path_str = path.to_string_lossy().to_string();
            let output = Command::new("pdftotext").args([&path_str, "-"]).output();
            match output {
                Ok(out) if out.status.success() => {
                    let txt = String::from_utf8_lossy(&out.stdout).to_string();
                    let cleaned = txt.split_whitespace().collect::<Vec<_>>().join(" ");
                    if cleaned.is_empty() {
                        return Err(BackendError::Validation(format!(
                            "PAR-2001 empty extraction for {}",
                            path.display()
                        )));
                    }
                    Ok(cleaned)
                }
                _ => Err(BackendError::Validation(format!(
                    "PAR-2001 local extraction failed for {}",
                    path.display()
                ))),
            }
        }
        _ => Err(BackendError::Validation(format!(
            "PAR-2001 unsupported local extraction extension '{}' for {}",
            ext,
            path.display()
        ))),
    }
}

fn detect_published_at(content: &str) -> Option<String> {
    let tokens = content.split_whitespace().collect::<Vec<_>>();
    for i in 0..tokens.len() {
        if let Some(ts) = parse_ts_candidate(tokens[i]) {
            return Some(ts);
        }
        if i + 1 < tokens.len() {
            let joined = format!("{} {}", tokens[i], tokens[i + 1]);
            if let Some(ts) = parse_ts_candidate(&joined) {
                return Some(ts);
            }
        }
    }
    None
}

fn parse_ts_candidate(raw: &str) -> Option<String> {
    let mut s = raw
        .trim_matches(|c: char| ",.;:!?()[]{}\"'".contains(c))
        .replace('/', "-")
        .replace('T', " ");
    if s.len() >= 19 {
        s = s[..19].to_string();
        if is_yyyy_mm_dd_hh_mm_ss(&s) {
            return normalize_ts(Some(&s));
        }
    }
    None
}

fn is_yyyy_mm_dd_hh_mm_ss(v: &str) -> bool {
    let b = v.as_bytes();
    if b.len() != 19 {
        return false;
    }
    let digit = |i: usize| b[i].is_ascii_digit();
    digit(0)
        && digit(1)
        && digit(2)
        && digit(3)
        && b[4] == b'-'
        && digit(5)
        && digit(6)
        && b[7] == b'-'
        && digit(8)
        && digit(9)
        && b[10] == b' '
        && digit(11)
        && digit(12)
        && b[13] == b':'
        && digit(14)
        && digit(15)
        && b[16] == b':'
        && digit(17)
        && digit(18)
}

fn file_modified_at(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let dt: DateTime<Local> = modified.into();
    Some(dt.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn strip_html(input: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in input.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn short_hash(bytes: &[u8]) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

fn map_error_code(e: &BackendError) -> &'static str {
    let msg = e.to_string();
    if msg.contains("ING-1001") {
        "ING-1001"
    } else if msg.contains("PAR-2001") {
        "PAR-2001"
    } else {
        "IPC-6001"
    }
}
