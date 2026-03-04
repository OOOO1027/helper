import type { BackendReviewItem, ReviewItem, ReviewTab } from "../types/contracts";

function mapStateToTab(state: string): ReviewTab {
  const normalized = state.toLowerCase();
  if (normalized === "pending" || normalized === "review" || normalized === "processing") {
    return "pending";
  }
  if (
    normalized.includes("failed") ||
    normalized.includes("error") ||
    normalized === "exception" ||
    normalized === "rejected"
  ) {
    return "exception";
  }
  return "done";
}

function normalizeTitle(raw?: string): string | null {
  const title = raw?.trim();
  if (!title) {
    return null;
  }
  if (title.toLowerCase() === "title") {
    return null;
  }
  return title;
}

function fallbackTitle(url: string | undefined, id: string): string {
  if (url) {
    try {
      const parsed = new URL(url);
      const host = parsed.hostname.replace(/^www\./, "");
      if (host.includes("xiaohongshu.com")) {
        return `小红书内容 ${id.slice(0, 8)}`;
      }
      return `来源 ${host}`;
    } catch {
      return `待审核内容 ${id.slice(0, 8)}`;
    }
  }
  return `待审核内容 ${id.slice(0, 8)}`;
}

function humanizeReason(reason: string): string {
  const normalized = reason.trim().toLowerCase();
  if (!normalized) {
    return "触发人工复核";
  }
  if (normalized === "xhs source requires manual review") {
    return "小红书来源内容，默认进入人工审核";
  }
  if (normalized === "xhs anomaly requires manual review") {
    return "小红书内容出现异常信号，需要人工审核";
  }
  if (normalized === "xhs auto-pass normal item") {
    return "系统自动通过（小红书正常项）";
  }
  if (normalized === "c < 0.78 or valuescore >= 0.85") {
    return "模型判定不稳定或价值较高，需要人工确认";
  }
  if (normalized === "manual:approved") {
    return "人工处理：已通过";
  }
  if (normalized === "manual:rejected") {
    return "人工处理：已驳回";
  }
  if (normalized === "manual:duplicate") {
    return "人工处理：标记重复";
  }
  return reason;
}

function parseStringList(raw: unknown): string[] {
  if (Array.isArray(raw)) {
    return raw
      .map((item) => (typeof item === "string" ? item.trim() : ""))
      .filter((item) => item.length > 0);
  }
  if (typeof raw === "string") {
    const normalized = raw.trim();
    if (!normalized) {
      return [];
    }
    if (normalized.startsWith("[") && normalized.endsWith("]")) {
      try {
        const parsed = JSON.parse(normalized) as unknown;
        if (Array.isArray(parsed)) {
          return parsed
            .map((item) => (typeof item === "string" ? item.trim() : ""))
            .filter((item) => item.length > 0);
        }
      } catch {
        // Fall back to delimiter split below.
      }
    }
    return normalized
      .split(/[\n,，;；|]/)
      .map((item) => item.trim())
      .filter((item) => item.length > 0);
  }
  return [];
}

function pickSummary(item: BackendReviewItem): string | null {
  const first = item.conclusion_summary?.trim();
  if (first) {
    return first;
  }
  const second = item.summary?.trim();
  if (second) {
    return second;
  }
  return null;
}

export function toReviewItem(item: BackendReviewItem): ReviewItem {
  const keyPoints = parseStringList(item.key_points ?? item.key_points_json ?? null);
  const tags = parseStringList(item.tags ?? item.tags_json ?? null);
  const imageUrls = parseStringList(item.image_urls ?? item.image_urls_json ?? null);
  const coverUrl = item.cover_url?.trim() || null;
  return {
    id: item.id,
    normalizedId: item.normalized_item_id,
    title: normalizeTitle(item.title) ?? fallbackTitle(item.url, item.id),
    url: item.url,
    priority: item.priority,
    qualityBand: item.quality_band || "unknown",
    reason: humanizeReason(item.reason),
    state: item.state,
    publishState: item.publish_state || "pending",
    conclusionSummary: pickSummary(item),
    keyPoints,
    tags,
    coverUrl,
    imageUrls,
    tab: mapStateToTab(item.state),
  };
}
