use chrono::{DateTime, Datelike, Utc, Weekday};
use chrono_tz::Tz;
use std::collections::HashSet;

pub const DEFAULT_CATEGORIES: [&str; 7] = ["学习", "工作", "生活", "健康", "财务", "灵感", "Inbox"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryRouteDecision {
    pub category: String,
    pub reason: Option<String>,
    pub created_new_custom_category: bool,
}

pub fn route_category(
    labels: &[String],
    confidence: f64,
    route_min_confidence: f64,
    unknown_category: &str,
    default_categories: &[String],
) -> CategoryRouteDecision {
    route_category_with_growth_limit(
        labels,
        confidence,
        route_min_confidence,
        unknown_category,
        default_categories,
        &HashSet::new(),
        0,
    )
}

pub fn route_category_with_growth_limit(
    labels: &[String],
    confidence: f64,
    route_min_confidence: f64,
    unknown_category: &str,
    default_categories: &[String],
    known_custom_categories: &HashSet<String>,
    category_growth_limit: u32,
) -> CategoryRouteDecision {
    if confidence < route_min_confidence {
        return CategoryRouteDecision {
            category: unknown_category.to_string(),
            reason: Some("low_confidence".to_string()),
            created_new_custom_category: false,
        };
    }
    let default_set = default_categories
        .iter()
        .map(|v| normalize_key(v))
        .collect::<HashSet<_>>();
    let unknown_key = normalize_key(unknown_category);

    for label in labels {
        if let Some(matched) = default_categories
            .iter()
            .find(|item| item.eq_ignore_ascii_case(label))
        {
            return CategoryRouteDecision {
                category: matched.clone(),
                reason: None,
                created_new_custom_category: false,
            };
        }
    }
    let fallback = labels
        .first()
        .cloned()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| unknown_category.to_string());
    let fallback_key = normalize_key(&fallback);
    if fallback_key == unknown_key || fallback_key.is_empty() {
        return CategoryRouteDecision {
            category: unknown_category.to_string(),
            reason: None,
            created_new_custom_category: false,
        };
    }
    if default_set.contains(&fallback_key) {
        let matched = default_categories
            .iter()
            .find(|v| normalize_key(v) == fallback_key)
            .cloned()
            .unwrap_or_else(|| fallback.clone());
        return CategoryRouteDecision {
            category: matched,
            reason: None,
            created_new_custom_category: false,
        };
    }
    if category_growth_limit == 0 {
        return CategoryRouteDecision {
            category: unknown_category.to_string(),
            reason: Some("growth_limit_exceeded".to_string()),
            created_new_custom_category: false,
        };
    }
    if known_custom_categories.contains(&fallback_key) {
        return CategoryRouteDecision {
            category: fallback,
            reason: None,
            created_new_custom_category: false,
        };
    }
    if known_custom_categories.len() >= category_growth_limit as usize {
        return CategoryRouteDecision {
            category: unknown_category.to_string(),
            reason: Some("growth_limit_exceeded".to_string()),
            created_new_custom_category: false,
        };
    }
    CategoryRouteDecision {
        category: fallback,
        reason: None,
        created_new_custom_category: true,
    }
}

pub fn week_key_and_title(
    ts_utc: DateTime<Utc>,
    timezone: &str,
) -> Result<(String, String), String> {
    let tz: Tz = timezone
        .parse()
        .map_err(|_| format!("invalid timezone: {}", timezone.trim()))?;
    let local = ts_utc.with_timezone(&tz);
    let iso = local.iso_week();
    let key = format!("{}-W{:02}", iso.year(), iso.week());
    let title = format!("{}年第{:02}周", iso.year(), iso.week());
    Ok((key, title))
}

pub fn clean_item_title(title: &str) -> String {
    let compact = title
        .split_whitespace()
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = compact.trim();
    if trimmed.is_empty() {
        return "Untitled".to_string();
    }
    trimmed.chars().take(80).collect()
}

pub fn default_category_strings() -> Vec<String> {
    DEFAULT_CATEGORIES
        .iter()
        .map(|v| (*v).to_string())
        .collect()
}

pub fn default_weekday_start() -> Weekday {
    Weekday::Mon
}

pub fn normalize_key(input: &str) -> String {
    input.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{
        clean_item_title, default_category_strings, route_category_with_growth_limit,
        week_key_and_title,
    };
    use chrono::{DateTime, Utc};
    use std::collections::HashSet;

    #[test]
    fn route_to_inbox_when_low_confidence() {
        let decision = route_category_with_growth_limit(
            &["Reading List".to_string()],
            0.6,
            0.72,
            "Inbox",
            &default_category_strings(),
            &HashSet::new(),
            10,
        );
        assert_eq!(decision.category, "Inbox");
    }

    #[test]
    fn week_key_cross_year_boundary() {
        let ts = DateTime::parse_from_rfc3339("2025-12-29T16:10:00Z")
            .expect("ts")
            .with_timezone(&Utc);
        let (key, title) = week_key_and_title(ts, "Asia/Shanghai").expect("week key");
        assert_eq!(key, "2026-W01");
        assert_eq!(title, "2026年第01周");
    }

    #[test]
    fn week_key_changes_with_timezone() {
        let ts = DateTime::parse_from_rfc3339("2026-01-05T00:30:00Z")
            .expect("ts")
            .with_timezone(&Utc);
        let (utc_key, _) = week_key_and_title(ts, "UTC").expect("utc week");
        let (la_key, _) = week_key_and_title(ts, "America/Los_Angeles").expect("la week");
        assert_eq!(utc_key, "2026-W02");
        assert_eq!(la_key, "2026-W01");
    }

    #[test]
    fn week_key_rejects_invalid_timezone() {
        let ts = DateTime::parse_from_rfc3339("2026-01-05T00:30:00Z")
            .expect("ts")
            .with_timezone(&Utc);
        let err = week_key_and_title(ts, "Asia/Nowhere").expect_err("invalid tz");
        assert!(err.contains("invalid timezone"));
    }

    #[test]
    fn growth_limit_zero_routes_custom_to_unknown() {
        let decision = route_category_with_growth_limit(
            &["Custom One".to_string()],
            0.9,
            0.72,
            "Inbox",
            &default_category_strings(),
            &HashSet::new(),
            0,
        );
        assert_eq!(decision.category, "Inbox");
        assert_eq!(decision.reason.as_deref(), Some("growth_limit_exceeded"));
    }

    #[test]
    fn growth_limit_allows_custom_when_under_limit() {
        let mut known = HashSet::new();
        known.insert("custom-a".to_string());
        let decision = route_category_with_growth_limit(
            &["Custom B".to_string()],
            0.9,
            0.72,
            "Inbox",
            &default_category_strings(),
            &known,
            3,
        );
        assert_eq!(decision.category, "Custom B");
        assert!(decision.created_new_custom_category);
    }

    #[test]
    fn growth_limit_routes_to_unknown_when_limit_reached() {
        let mut known = HashSet::new();
        known.insert("custom-a".to_string());
        let decision = route_category_with_growth_limit(
            &["Custom B".to_string()],
            0.9,
            0.72,
            "Inbox",
            &default_category_strings(),
            &known,
            1,
        );
        assert_eq!(decision.category, "Inbox");
        assert_eq!(decision.reason.as_deref(), Some("growth_limit_exceeded"));
    }

    #[test]
    fn clean_title_truncates_to_80_chars() {
        let title = clean_item_title(&"a".repeat(120));
        assert_eq!(title.chars().count(), 80);
    }
}
