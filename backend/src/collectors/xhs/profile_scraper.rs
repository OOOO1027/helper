use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use crate::collectors::{CollectWindow, CollectedItem};
use crate::{BackendError, Result};

use super::utils::{
    escape_for_applescript, normalize_content_text, normalize_profile_item_url, read_env_u32,
    run_osascript, truncate_chars, within_window,
};
use super::XhsCollector;

#[derive(Debug, Deserialize)]
struct BrowserProfileItem {
    id: String,
    href: String,
    #[serde(default)]
    text: String,
}

impl XhsCollector {
    pub(super) async fn collect_from_profile_tabs(
        &self,
        window: &CollectWindow,
        stop_external_ids: &HashSet<String>,
    ) -> Result<Vec<CollectedItem>> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let rounds = read_env_u32("XHS_PROFILE_ROUNDS", 12).clamp(4, 30);
        let max_items = read_env_u32("XHS_PROFILE_MAX_ITEMS", 120).clamp(20, 500);
        for tab in ["点赞", "收藏"] {
            let items = self.scrape_profile_tab_with_chrome(tab, rounds, max_items)?;
            for raw in items {
                let id = raw.id.trim().to_string();
                if id.is_empty() {
                    continue;
                }
                if stop_external_ids.contains(&id) || !seen.insert(id.clone()) {
                    continue;
                }
                let source_url = normalize_profile_item_url(&raw.href, &id);
                let content_text = normalize_content_text(&raw.text);
                let title = content_text
                    .lines()
                    .next()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .map(|line| truncate_chars(&line, 80));
                let content_raw = if content_text.is_empty() {
                    format!("xhs note {id}")
                } else {
                    content_text
                };
                let item = CollectedItem {
                    external_id: id,
                    source_url,
                    title,
                    content_raw,
                    published_at: None,
                    cover_url: None,
                    image_urls: Vec::new(),
                };
                if within_window(window, item.published_at.as_deref()) {
                    out.push(item);
                }
            }
        }
        if out.is_empty() {
            return Err(BackendError::Validation(
                "xhs profile tab scrape returned no items: ensure account is logged in and profile tabs (点赞/收藏) are visible".to_string(),
            ));
        }
        Ok(out)
    }

    fn scrape_profile_tab_with_chrome(
        &self,
        tab_label: &str,
        rounds: u32,
        max_items: u32,
    ) -> Result<Vec<BrowserProfileItem>> {
        let tab_json = serde_json::to_string(tab_label)
            .map_err(|e| BackendError::Internal(format!("xhs tab label encode failed: {e}")))?;
        let init_js = format!(
            r#"(() => {{
  const normalize = (s) => String(s || '').replace(/\s+/g, '').trim();
  const clickByText = (targets, selectors = 'a,button,div,span') => {{
    const wanted = new Set(targets.map(normalize));
    const nodes = Array.from(document.querySelectorAll(selectors));
    for (const node of nodes) {{
      const txt = normalize(node.innerText || node.textContent || '');
      if (!txt || !wanted.has(txt)) continue;
      try {{
        node.click();
        return true;
      }} catch (_) {{}}
    }}
    return false;
  }};
  const findProfileHref = () => {{
    const anchors = Array.from(document.querySelectorAll('a[href*="/user/profile/"]'));
    if (anchors.length === 0) return '';
    const me = anchors.find((a) => normalize(a.innerText || a.textContent || '') === '我');
    const picked = me || anchors[0];
    return String((picked && picked.href) || '');
  }};
  const closeHintDialog = () => clickByText(['我知道了', '知道了', '关闭']);
  closeHintDialog();
  const profileHref = findProfileHref();
  const inProfile = location.href.indexOf('/user/profile/') >= 0;
  if (!inProfile && profileHref) {{
    location.href = profileHref;
    return JSON.stringify({{ stage: 'goto_profile', profileHref }});
  }}
  clickByText(['我']);
  closeHintDialog();
  const tab = {tab_json};
  const tabHit = clickByText([tab]);
  const activeTab = (() => {{
    const active =
      document.querySelector('.reds-tab-item.active') ||
      document.querySelector('[class*="tab"][class*="active"]');
    return normalize(active && (active.innerText || active.textContent || ''));
  }})();
  if (activeTab && activeTab !== normalize(tab)) {{
    clickByText([tab]);
  }}
  window.__helperXhsItems = [];
  window.__helperXhsSeen = {{}};
  window.__helperXhsTargetTab = normalize(tab);
  return JSON.stringify({{ tabHit, href: location.href }});
}})()"#
        );
        let collect_js = r#"(() => {
  const normalizeText = (s) => String(s || '').replace(/\s+/g, ' ').trim();
  const normalize = (s) => String(s || '').replace(/\s+/g, '').trim();
  const clickByText = (targets, selectors = 'button,div,span,a') => {
    const wanted = new Set(targets.map(t => String(t || '').replace(/\s+/g, '').trim()));
    const nodes = Array.from(document.querySelectorAll(selectors));
    for (const node of nodes) {
      const txt = String(node.innerText || node.textContent || '').replace(/\s+/g, '').trim();
      if (!txt || !wanted.has(txt)) continue;
      try { node.click(); return true; } catch (_) {}
    }
    return false;
  };
  if (location.href.indexOf('/user/profile/') < 0) {
    return JSON.stringify({ error: 'not_in_profile', href: location.href });
  }
  clickByText(['我知道了', '知道了', '关闭']);
  const targetTab = String(window.__helperXhsTargetTab || '');
  const activeNode =
    document.querySelector('.reds-tab-item.active') ||
    document.querySelector('[class*="tab"][class*="active"]');
  const activeTab = normalize(activeNode && (activeNode.innerText || activeNode.textContent || ''));
  if (targetTab && activeTab !== targetTab) {
    clickByText([targetTab]);
  }
  if (!Array.isArray(window.__helperXhsItems)) window.__helperXhsItems = [];
  if (!window.__helperXhsSeen || typeof window.__helperXhsSeen !== 'object') window.__helperXhsSeen = {};
  const root =
    document.querySelector('.note-scroller') ||
    document.querySelector('.profile-content') ||
    document.querySelector('.user-page') ||
    document.body;
  const anchors = Array.from(root.querySelectorAll('a'));
  for (const a of anchors) {
    const href = String(a.href || '');
    if (!href.includes('/explore/')) continue;
    const m = href.match(/\/explore\/([0-9a-zA-Z]+)/);
    if (!m) continue;
    const id = m[1];
    if (!id || window.__helperXhsSeen[id]) continue;
    const container = a.closest('section,article,div') || a;
    const text = normalizeText(
      a.getAttribute('title') || a.innerText || container.innerText || ''
    ).slice(0, 220);
    window.__helperXhsSeen[id] = 1;
    window.__helperXhsItems.push({ id, href, text });
  }
  return JSON.stringify({ count: window.__helperXhsItems.length, href: location.href, activeTab: activeTab || targetTab || '' });
})()"#;
        let scroll_js =
            r#"(() => { window.scrollTo(0, document.body.scrollHeight || 0); return 'ok'; })()"#;
        let export_js = r#"(() => JSON.stringify(window.__helperXhsItems || []))()"#;
        let verify_js = r#"(() => JSON.stringify({ inProfile: location.href.indexOf('/user/profile/') >= 0, href: location.href }))()"#;
        let script = format!(
            r#"tell application "Google Chrome"
  if it is not running then
    launch
  end if
  if (count of windows) = 0 then
    make new window
  end if
  if (count of tabs of front window) = 0 then
    make new tab at end of tabs of front window with properties {{URL:"https://www.xiaohongshu.com/explore"}}
  end if
  set targetTab to active tab of front window
  if URL of targetTab does not contain "xiaohongshu.com" then
    set URL of targetTab to "https://www.xiaohongshu.com/explore"
    delay 2
  end if
  execute targetTab javascript "{init_js_escaped}"
  delay 2.2
  execute targetTab javascript "{init_js_escaped}"
  delay 1.2
  set verifyResult to execute targetTab javascript "{verify_js_escaped}"
  if verifyResult does not contain "\"inProfile\":true" then
    return "{{\"error\":\"profile_navigation_failed\",\"detail\":" & quoted form of verifyResult & "}}"
  end if
  set lastCount to -1
  set stagnantRounds to 0
  repeat {rounds} times
    set currentResultText to execute targetTab javascript "{collect_js_escaped}"
    if currentResultText contains "\"error\"" then
      return currentResultText
    end if
    set currentCountText to currentResultText
    if currentResultText contains "\"count\":" then
      set AppleScript's text item delimiters to "\"count\":"
      set tailPart to text item 2 of currentResultText
      set AppleScript's text item delimiters to ","
      set currentCountText to text item 1 of tailPart
      set AppleScript's text item delimiters to ""
    end if
    set currentCount to lastCount
    try
      set currentCount to currentCountText as integer
    end try
    if currentCount is greater than or equal to {max_items} then
      exit repeat
    end if
    if currentCount is equal to lastCount then
      set stagnantRounds to stagnantRounds + 1
    else
      set stagnantRounds to 0
      set lastCount to currentCount
    end if
    if stagnantRounds is greater than or equal to 3 then
      exit repeat
    end if
    execute targetTab javascript "{scroll_js_escaped}"
    delay 0.7
  end repeat
  set payload to execute targetTab javascript "{export_js_escaped}"
  return payload
end tell"#,
            rounds = rounds,
            max_items = max_items,
            init_js_escaped = escape_for_applescript(&init_js),
            collect_js_escaped = escape_for_applescript(collect_js),
            scroll_js_escaped = escape_for_applescript(scroll_js),
            export_js_escaped = escape_for_applescript(export_js),
            verify_js_escaped = escape_for_applescript(verify_js),
        );
        let output = run_osascript(&script)?;
        let trimmed = output.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("undefined") || trimmed == "null" {
            return Err(BackendError::Validation(format!(
                "xhs profile tab scrape empty: tab={tab_label}"
            )));
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|e| {
            let preview: String = trimmed.chars().take(120).collect();
            BackendError::Validation(format!(
                "xhs profile tab scrape decode failed: {e}; tab={tab_label}; output_preview={preview}"
            ))
        })?;
        if let Some(err) = value.get("error").and_then(Value::as_str) {
            let detail = value
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            return Err(BackendError::Validation(format!(
                "xhs profile tab scrape failed: tab={tab_label}, error={err}, detail={detail}"
            )));
        }
        serde_json::from_value(value).map_err(|e| {
            BackendError::Validation(format!(
                "xhs profile tab scrape payload invalid: {e}; tab={tab_label}"
            ))
        })
    }
}
