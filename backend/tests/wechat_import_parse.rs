use helper_backend::app_core::AppCore;
use helper_backend::collectors::wechat_import::parse_wechat_files;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file(name: &str, content: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let mut p = std::env::temp_dir();
    let file_name = match name.rsplit_once('.') {
        Some((stem, ext)) => format!("helper_backend_{stem}_{ts}.{ext}"),
        None => format!("helper_backend_{name}_{ts}"),
    };
    p.push(file_name);
    fs::write(&p, content).expect("write temp file");
    p.to_string_lossy().to_string()
}

fn remove_if_exists(path: &str) {
    let p = PathBuf::from(path);
    if p.exists() {
        let _ = fs::remove_file(p);
    }
}

#[test]
fn parse_wechat_txt_and_detect_duplicate() {
    let p1 = temp_file(
        "wx1.txt",
        "发布时间 2026-02-25 21:30:00 内容 https://example.com/post?id=1",
    );
    let p2 = temp_file(
        "wx2.txt",
        "发布时间 2026-02-25 21:30:00 内容 https://example.com/post?id=1",
    );

    let parsed = parse_wechat_files(&[p1.clone(), p2.clone()]);
    assert_eq!(parsed.items.len(), 1);
    assert_eq!(parsed.duplicates, 1);
    assert_eq!(parsed.errors.len(), 0);
    assert_eq!(
        parsed.items[0].source_url.as_deref(),
        Some("https://example.com/post")
    );
    assert_eq!(
        parsed.items[0].published_at.as_deref(),
        Some("2026-02-25 21:30:00")
    );

    remove_if_exists(&p1);
    remove_if_exists(&p2);
}

#[test]
fn parse_wechat_unsupported_file_type() {
    let p = temp_file("wx3.unsupported", "dummy");
    let parsed = parse_wechat_files(&[p.clone()]);
    assert_eq!(parsed.items.len(), 0);
    assert_eq!(parsed.errors.len(), 1);
    assert_eq!(parsed.errors[0].code, "PAR-2001");
    remove_if_exists(&p);
}

#[test]
fn app_core_import_uses_parser_summary() {
    let p1 = temp_file("wx4.txt", "A https://a.com");
    let p2 = temp_file("wx5.txt", "A https://a.com");
    let p3 = temp_file("wx6.unsupported", "dummy");
    let app = AppCore::default();
    let summary = app
        .import_wechat_files(&[p1.clone(), p2.clone(), p3.clone()])
        .expect("import");
    assert_eq!(summary.total, 3);
    assert_eq!(summary.imported, 1);
    assert_eq!(summary.duplicates, 1);
    assert_eq!(summary.failed, 1);
    remove_if_exists(&p1);
    remove_if_exists(&p2);
    remove_if_exists(&p3);
}
