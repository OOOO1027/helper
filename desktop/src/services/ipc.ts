import type {
  AiUsageMonthlySummary,
  ApiResponse,
  BackendReviewItem,
  BudgetStatus,
  CollectResult,
  CollectionInsight,
  DashboardMetrics,
  DateRange,
  GetReviewQueueReq,
  GetReviewItemsReq,
  IngestQueueItem,
  ImportPersistSummary,
  LocalToolStatus,
  PageReq,
  Paged,
  PublishHistoryFilter,
  PublishHistoryItem,
  PublishResult,
  PublishTask,
  RetryResult,
  ReviewItem,
  SourceStateMarkResult,
  SyncRunSummary,
  SyncConfigSnapshot,
  SyncDailyStat,
  SyncLog,
  UpdateReviewItemReq
} from "../types/contracts";
import { toReviewItem } from "./adapters";

class IpcInvokeError extends Error {
  public readonly code: string;
  public readonly retryable: boolean;

  constructor(code: string, message: string, retryable: boolean) {
    super(message);
    this.code = code;
    this.retryable = retryable;
  }
}

function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function toUserMessage(error: unknown, fallback: string): string {
  if (error instanceof IpcInvokeError) {
    if (error.code === "IPC-LOCAL") {
      return "当前是本地预览模式，部分桌面能力不可用。";
    }
    if (error.message.includes("NOTION_ROOT_PAGE_ID is required")) {
      return "未检测到 Notion 根页面配置。下一步：在运行环境补充 NOTION_ROOT_PAGE_ID 后重试。";
    }
    if (error.message.includes("NOTION_SYNC_MODE must be")) {
      return "同步模式配置无效。下一步：将 NOTION_SYNC_MODE 设置为 page_tree 或 database 后重试。";
    }
    if (error.message.includes("NOTION_TOKEN is required")) {
      return "Notion 访问凭证缺失。下一步：先配置 NOTION_TOKEN，再执行同步。";
    }
    if (
      error.message.includes("API token is invalid") ||
      (error.message.includes("401 Unauthorized") && error.message.includes("notion"))
    ) {
      return "Notion Token 无效。下一步：将 NOTION_TOKEN 设置为原始 token（不要包含 Bearer 前缀），重启应用后重试。";
    }
    if (error.message.includes("NOTION_DATABASE_ID is required")) {
      return "Notion 数据库 ID 缺失。下一步：在 database 模式下配置 NOTION_DATABASE_ID 后重试。";
    }
    if (
      error.message.includes("NOTION_DATABASE_ID") ||
      error.message.includes("NOTION_TOKEN")
    ) {
      return "Notion 同步配置不完整。下一步：补齐必要配置后重试。";
    }
    if (
      error.message.includes("HELPER_DB_PATH is required") ||
      error.message.includes("HELPER_DB_KEY is required")
    ) {
      return "本地数据存储尚未就绪，请检查应用环境后再重试。";
    }
    if (error.message.includes("invalid timezone")) {
      return "同步时区配置无效。下一步：将 NOTION_TREE_TIMEZONE 设为有效时区（如 Asia/Shanghai）后重试。";
    }
    if (error.message.includes("NOTION_TREE_CATEGORY_GROWTH_LIMIT must be >= 0")) {
      return "分类增长上限配置无效。下一步：将 NOTION_TREE_CATEGORY_GROWTH_LIMIT 设为不小于 0 的整数。";
    }
    if (
      error.message.includes("range.from and range.to must not be empty") ||
      error.message.includes("range.from invalid format") ||
      error.message.includes("range.to invalid format")
    ) {
      return "时间范围无效，请重新选择后重试。";
    }
    if (error.message.includes("page and page_size must be positive")) {
      return "分页参数无效，请刷新页面后重试。";
    }
    if (error.message.includes("status must be one of")) {
      return "筛选条件无效，请重置筛选后重试。";
    }
    if (
      error.message.includes("Allow JavaScript from Apple Events") ||
      error.message.includes("xhs collector requires enabling Chrome menu")
    ) {
      return "请在 Chrome 的“显示 > 开发者 > 允许 Apple 事件中的 JavaScript”中开启权限后重试。";
    }
    if (error.message.includes("xhs osascript failed")) {
      return "小红书同步调用 Chrome 失败，请确认 Chrome 已启动并已登录小红书后重试。";
    }
    if (error.message.includes("xhs request rejected: http_status=")) {
      return "小红书接口拒绝了本次请求。下一步：先确认账号仍处于登录状态，再重试；若持续失败，请改用 XHS_COOKIE。";
    }
    if (error.message.includes("xhs browser request rejected: http_status=")) {
      return "浏览器抓取小红书返回异常状态。下一步：先刷新并登录小红书页面，再重试同步。";
    }
    if (error.message.includes("xhs browser returned html page")) {
      return "当前返回的是登录/验证页面，不是数据结果。下一步：先在 Chrome 完成登录与验证，再重试。";
    }
    if (error.message.includes("xhs profile tab scrape returned no items")) {
      return "未抓取到点赞/收藏条目。下一步：确认小红书页面可见“点赞/收藏”标签并有内容后重试。";
    }
    if (
      error.message.includes("xhs browser envelope decode failed") ||
      error.message.includes("xhs browser body decode failed") ||
      error.message.includes("xhs response json decode failed")
    ) {
      return "小红书返回内容解析失败。下一步：刷新小红书页面后重试；若仍失败，建议切换到 XHS_COOKIE 方式。";
    }
    if (error.message.includes("xhs browser fetch returned empty payload")) {
      return "小红书同步返回空结果。下一步：先确认 Chrome 已登录小红书并停留在小红书域名页面，再重试；若仍失败，改用 XHS_COOKIE 方式。";
    }
    if (error.message.includes("validation failed")) {
      const detail = error.message.replace(/^validation failed:\s*/i, "").trim();
      if (detail.length > 0) {
        return `参数校验失败：${detail}`;
      }
      return "输入参数不符合要求，请检查后重试。";
    }
    if (error.message.includes("paths must contain at least one file")) {
      return "请先选择至少一个微信导出文件。";
    }
    if (error.message.includes("xhs collector blocked")) {
      return "小红书同步受限：请在 Chrome 开启“允许 Apple 事件中的 JavaScript”，或在环境变量配置 XHS_COOKIE 后重试。";
    }
    return error.message || fallback;
  }

  if (error instanceof Error) {
    return error.message || fallback;
  }
  return fallback;
}

async function invokeWithEnvelope<T>(
  command: string,
  payload?: Record<string, unknown>
): Promise<T> {
  if (!isTauriRuntime()) {
    throw new IpcInvokeError("IPC-LOCAL", "当前不在 Tauri 运行时", false);
  }

  const { invoke } = await import("@tauri-apps/api/core");
  const response = await invoke<ApiResponse<T>>(command, payload);

  if (!response.ok || response.data === undefined || response.data === null) {
    const err = response.error;
    throw new IpcInvokeError(
      err?.code ?? "IPC-UNKNOWN",
      err?.message ?? "unknown ipc error",
      err?.retryable ?? false
    );
  }

  return response.data;
}

function localMockPaged(page: PageReq): Paged<ReviewItem> {
  return {
    page: page.page,
    page_size: page.page_size,
    total: 1,
    items: [
      {
        id: "mock-1",
        normalizedId: "normalized-mock-1",
        title: "示例内容：AI 复核策略",
        priority: 0.58,
        qualityBand: "mid",
        reason: "C < 0.78 OR ValueScore >= 0.85",
        state: "pending",
        publishState: "pending",
        tab: "pending"
      }
    ]
  };
}

export async function getDashboardMetrics(): Promise<DashboardMetrics> {
  if (!isTauriRuntime()) {
    return {
      today_collected: 25,
      pending_review: 6,
      classification_accuracy: 0.81,
      summary_usability: 0.76,
      budget_usage_ratio: 0.32
    };
  }
  return invokeWithEnvelope<DashboardMetrics>("get_dashboard_metrics");
}

export async function getCollectionInsight(): Promise<CollectionInsight> {
  if (!isTauriRuntime()) {
    return {
      day: "2026-02-26",
      today_total: 25,
      sources: [
        { source: "xhs", count: 20 },
        { source: "wechat", count: 5 }
      ],
      review_pending: 6,
      review_done: 10,
      review_rejected: 1,
      direct_no_review: 8,
      sync_pending: 7,
      sync_success: 14,
      sync_failed: 1,
      sync_retry: 0,
      sync_not_started: 3,
      recent_items: []
    };
  }
  return invokeWithEnvelope<CollectionInsight>("get_collection_insight");
}

export async function getBudgetStatus(): Promise<BudgetStatus> {
  if (!isTauriRuntime()) {
    return {
      month: "2026-02",
      limit_cny: 100,
      used_cny: 32
    };
  }
  return invokeWithEnvelope<BudgetStatus>("get_budget_status");
}

export async function getAiUsageMonthlySummary(month?: string): Promise<AiUsageMonthlySummary> {
  if (!isTauriRuntime()) {
    return {
      month: "2026-02",
      budget_limit_cny: 100,
      used_cny: 18.6,
      usage_ratio: 0.186,
      remaining_cny: 81.4,
      calls: 240,
      tokens_in: 520000,
      tokens_out: 86000,
      avg_cost_per_call_cny: 18.6 / 240,
      models: [
        {
          model: "qwen-flash",
          calls: 240,
          tokens_in: 520000,
          tokens_out: 86000,
          cost_cny: 18.6
        }
      ],
      daily: [
        { day: "2026-02-26", calls: 42, tokens_in: 91000, tokens_out: 15000, cost_cny: 3.2 },
        { day: "2026-02-25", calls: 37, tokens_in: 82000, tokens_out: 14000, cost_cny: 2.9 },
        { day: "2026-02-24", calls: 33, tokens_in: 74000, tokens_out: 12000, cost_cny: 2.5 }
      ]
    };
  }
  return invokeWithEnvelope<AiUsageMonthlySummary>("get_ai_usage_monthly_summary", {
    req: { month }
  });
}

export async function importWechatAndPersist(paths: string[]): Promise<ImportPersistSummary> {
  if (!isTauriRuntime()) {
    const total = paths.length;
    return {
      batch_id: "mock-batch",
      total,
      parsed_ok: total,
      duplicates: 0,
      parse_failed: 0,
      persisted: total,
      persist_failed: 0
    };
  }
  return invokeWithEnvelope<ImportPersistSummary>("import_wechat_and_persist", {
    req: { paths }
  });
}

export async function pickWechatFiles(): Promise<string[]> {
  if (!isTauriRuntime()) {
    return [];
  }

  const { open } = await import("@tauri-apps/plugin-dialog");
  const pickerOptions = {
    multiple: true,
    title: "选择微信导出文件"
  } as const;
  const defaultPath = await resolveWechatPickerDefaultPath();
  const selected = await (async () => {
    if (!defaultPath) {
      return open(pickerOptions);
    }
    try {
      return await open({
        ...pickerOptions,
        defaultPath
      });
    } catch {
      return open(pickerOptions);
    }
  })();

  if (!selected) {
    return [];
  }
  if (Array.isArray(selected)) {
    const paths = selected.filter((item): item is string => typeof item === "string");
    rememberWechatPickerDir(paths);
    return paths;
  }
  if (typeof selected === "string") {
    rememberWechatPickerDir([selected]);
    return [selected];
  }
  return [];
}

export async function getReviewItems(req: GetReviewItemsReq): Promise<Paged<ReviewItem>> {
  if (!isTauriRuntime()) {
    return localMockPaged(req.page);
  }
  const data = await invokeWithEnvelope<Paged<BackendReviewItem>>("get_review_items", {
    req
  });
  return {
    ...data,
    items: data.items.map(toReviewItem)
  };
}

export async function getReviewQueue(req: GetReviewQueueReq): Promise<Paged<ReviewItem>> {
  if (!isTauriRuntime()) {
    return localMockPaged(req.page);
  }
  try {
    const data = await invokeWithEnvelope<Paged<BackendReviewItem>>("get_review_queue", { req });
    return {
      ...data,
      items: data.items.map(toReviewItem)
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const unknownCommand =
      message.includes("unknown command") ||
      message.includes("not managed") ||
      message.includes("command get_review_queue not found");
    if (unknownCommand) {
      return getReviewItems(req);
    }
    throw error;
  }
}

export async function updateReviewItem(req: UpdateReviewItemReq): Promise<ReviewItem> {
  if (!isTauriRuntime()) {
    return {
      id: req.id,
      normalizedId: "normalized-mock-1",
      title: "示例内容：AI 复核策略",
      priority: 0.45,
      qualityBand: "mid",
      reason: req.patch.note ?? "manual update",
      state: req.patch.state ?? "done",
      publishState: "pending",
      tab: "done"
    };
  }
  const data = await invokeWithEnvelope<BackendReviewItem>("update_review_item", {
    req
  });
  return toReviewItem(data);
}

export async function retryFailedItems(ids: string[]): Promise<RetryResult> {
  if (!isTauriRuntime()) {
    return {
      requested: ids.length,
      requeued: ids.length,
      ignored: 0
    };
  }
  return invokeWithEnvelope<RetryResult>("retry_failed_items", { req: { ids } });
}

export async function retryDeadLetters(ids: string[]): Promise<RetryResult> {
  if (!isTauriRuntime()) {
    return {
      requested: ids.length,
      requeued: ids.length,
      ignored: 0
    };
  }
  return invokeWithEnvelope<RetryResult>("retry_dead_letters", { req: { ids } });
}

export async function collectSource(
  source: string,
  since?: string,
  until?: string
): Promise<CollectResult> {
  if (!isTauriRuntime()) {
    return {
      source,
      fetched: 0,
      stored: 0
    };
  }
  return invokeWithEnvelope<CollectResult>("collect", {
    req: {
      source,
      since,
      until
    }
  });
}

export async function ingestXhsIncremental(): Promise<CollectResult> {
  if (!isTauriRuntime()) {
    return {
      source: "xhs",
      fetched: 0,
      stored: 0
    };
  }
  try {
    return await invokeWithEnvelope<CollectResult>("ingest_xhs_incremental");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const unknownCommand =
      message.includes("unknown command") ||
      message.includes("not managed") ||
      message.includes("command ingest_xhs_incremental not found");
    if (unknownCommand) {
      return collectSource("xhs");
    }
    throw error;
  }
}

export async function getIngestQueue(page: PageReq): Promise<Paged<IngestQueueItem>> {
  if (!isTauriRuntime()) {
    return {
      page: page.page,
      page_size: page.page_size,
      total: 0,
      items: []
    };
  }
  return invokeWithEnvelope<Paged<IngestQueueItem>>("get_ingest_queue", {
    req: { page }
  });
}

export async function runNotionSyncOnce(limit?: number): Promise<SyncRunSummary> {
  if (!isTauriRuntime()) {
    return {
      scanned: 0,
      attempted: 0,
      succeeded: 0,
      failed: 0,
      requeued: 0,
      dead_lettered: 0
    };
  }
  return invokeWithEnvelope<SyncRunSummary>("run_notion_sync_once", {
    req: { limit }
  });
}

export async function publishApprovedToNotion(limit?: number): Promise<PublishResult> {
  if (!isTauriRuntime()) {
    return {
      request_id: "mock-publish",
      attempted: 0,
      succeeded: 0,
      failed: 0,
      requeued: 0,
      dead_lettered: 0
    };
  }
  try {
    return await invokeWithEnvelope<PublishResult>("publish_approved_to_notion", {
      req: { limit }
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const unknownCommand =
      message.includes("unknown command") ||
      message.includes("not managed") ||
      message.includes("command publish_approved_to_notion not found");
    if (unknownCommand) {
      const summary = await runNotionSyncOnce(limit);
      return {
        request_id: "legacy-run_notion_sync_once",
        attempted: summary.attempted,
        succeeded: summary.succeeded,
        failed: summary.failed,
        requeued: summary.requeued,
        dead_lettered: summary.dead_lettered
      };
    }
    throw error;
  }
}

export async function getLocalToolStatus(): Promise<LocalToolStatus> {
  if (!isTauriRuntime()) {
    return { textutil: true, pdftotext: false };
  }
  return invokeWithEnvelope<LocalToolStatus>("get_local_tool_status");
}

export async function getSyncLogs(range: DateRange, page: PageReq): Promise<Paged<SyncLog>> {
  if (!isTauriRuntime()) {
    return {
      page: page.page,
      page_size: page.page_size,
      total: 1,
      items: [
        {
          id: "sync-log-1",
          state: "success",
          error_code: null,
          created_at: new Date().toISOString()
        }
      ]
    };
  }
  return invokeWithEnvelope<Paged<SyncLog>>("get_sync_logs", { req: { range, page } });
}

export async function getSyncDailyStats(
  range: DateRange,
  page: PageReq,
  filter?: { job_type?: string; status?: string }
): Promise<Paged<SyncDailyStat>> {
  if (!isTauriRuntime()) {
    return {
      page: page.page,
      page_size: page.page_size,
      total: 1,
      items: [
        {
          day: "2026-02-26",
          job_type: "notion_sync_once",
          run_count: 2,
          success_runs: 1,
          partial_runs: 1,
          failed_runs: 0,
          success_rate: 0.5,
          avg_success_count: 1,
          avg_fail_count: 0.5
        }
      ]
    };
  }
  return invokeWithEnvelope<Paged<SyncDailyStat>>("get_sync_daily_stats", {
    req: {
      range,
      page,
      job_type: filter?.job_type,
      status: filter?.status
    }
  });
}

export async function getPublishQueue(page: PageReq): Promise<Paged<PublishTask>> {
  if (!isTauriRuntime()) {
    return {
      page: page.page,
      page_size: page.page_size,
      total: 0,
      items: []
    };
  }
  try {
    return await invokeWithEnvelope<Paged<PublishTask>>("get_publish_queue", {
      req: { page }
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const unknownCommand =
      message.includes("unknown command") ||
      message.includes("not managed") ||
      message.includes("command get_publish_queue not found");
    if (unknownCommand) {
      return {
        page: page.page,
        page_size: page.page_size,
        total: 0,
        items: []
      };
    }
    throw error;
  }
}

export async function getPublishHistory(
  range: DateRange,
  page: PageReq,
  filters?: PublishHistoryFilter
): Promise<Paged<PublishHistoryItem>> {
  if (!isTauriRuntime()) {
    return {
      page: page.page,
      page_size: page.page_size,
      total: 0,
      items: []
    };
  }
  try {
    const data = await invokeWithEnvelope<Paged<unknown>>("get_publish_history", {
      req: {
        range,
        page,
        state: filters?.state,
        sync_mode: filters?.sync_mode,
        retryable: filters?.retryable
      }
    });
    return {
      ...data,
      items: Array.isArray(data.items) ? data.items.map((item) => normalizePublishHistoryItem(item)) : []
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const unknownCommand =
      message.includes("unknown command") ||
      message.includes("not managed") ||
      message.includes("command get_publish_history not found");
    if (unknownCommand) {
      return {
        page: page.page,
        page_size: page.page_size,
        total: 0,
        items: []
      };
    }
    throw error;
  }
}

function normalizePublishHistoryItem(input: unknown): PublishHistoryItem {
  const data = typeof input === "object" && input !== null ? (input as Record<string, unknown>) : {};
  const readString = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "string") {
        return value;
      }
    }
    return null;
  };
  const readNumber = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "number" && Number.isFinite(value)) {
        return value;
      }
      if (typeof value === "string") {
        const parsed = Number(value);
        if (Number.isFinite(parsed)) {
          return parsed;
        }
      }
    }
    return 0;
  };
  const readOptionalNumber = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "number" && Number.isFinite(value)) {
        return value;
      }
      if (typeof value === "string") {
        const parsed = Number(value);
        if (Number.isFinite(parsed)) {
          return parsed;
        }
      }
    }
    return null;
  };
  const readBool = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "boolean") {
        return value;
      }
      if (typeof value === "number") {
        return value !== 0;
      }
      if (typeof value === "string") {
        const normalized = value.trim().toLowerCase();
        if (normalized === "true" || normalized === "1") {
          return true;
        }
        if (normalized === "false" || normalized === "0") {
          return false;
        }
      }
    }
    return null;
  };
  const readList = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (Array.isArray(value)) {
        const items = value
          .map((entry) => (typeof entry === "string" ? entry.trim() : ""))
          .filter((entry) => entry.length > 0);
        if (items.length > 0) {
          return items;
        }
      }
      if (typeof value === "string") {
        const normalized = value.trim();
        if (!normalized) {
          continue;
        }
        if (normalized.startsWith("[") && normalized.endsWith("]")) {
          try {
            const parsed = JSON.parse(normalized) as unknown;
            if (Array.isArray(parsed)) {
              const items = parsed
                .map((entry) => (typeof entry === "string" ? entry.trim() : ""))
                .filter((entry) => entry.length > 0);
              if (items.length > 0) {
                return items;
              }
            }
          } catch {
            // Fallback to delimiter split below.
          }
        }
        const split = normalized
          .split(/[\n,，;；|]/)
          .map((entry) => entry.trim())
          .filter((entry) => entry.length > 0);
        if (split.length > 0) {
          return split;
        }
      }
    }
    return null;
  };

  return {
    id: readString("id", "request_id") ?? "unknown",
    state: readString("state", "status") ?? "unknown",
    error_code: readString("error_code"),
    created_at: readString("created_at", "updated_at") ?? new Date().toISOString(),
    success_count: Math.max(0, Math.floor(readNumber("success_count", "succeeded"))),
    fail_count: Math.max(0, Math.floor(readNumber("fail_count", "failed"))),
    pipeline_stage: readString("pipeline_stage", "stage"),
    sync_mode: readString("sync_mode", "mode"),
    retryable: readBool("retryable"),
    category: readString("category", "route_category", "category_name"),
    week_key: readString("week_key", "week", "route_week"),
    route_reason: readString("route_reason"),
    conclusion_summary: readString("conclusion_summary", "summary"),
    key_points: readList("key_points", "key_points_json"),
    tags: readList("tags", "tags_json"),
    cover_url: readString("cover_url"),
    image_urls: readList("image_urls", "image_urls_json"),
    quality_state: readString("quality_state"),
    quality_score: readOptionalNumber("quality_score"),
    degraded_fields: readList("degraded_fields", "degraded_fields_json")
  };
}

export async function markSourceInactive(itemId: string): Promise<SourceStateMarkResult> {
  if (!isTauriRuntime()) {
    return {
      item_id: itemId,
      active_state: "inactive",
      updated_at: new Date().toISOString()
    };
  }
  return invokeWithEnvelope<SourceStateMarkResult>("mark_source_inactive", {
    req: { item_id: itemId }
  });
}

function normalizeSyncConfigSnapshot(input: unknown, source: "ipc" | "fallback"): SyncConfigSnapshot {
  const data = typeof input === "object" && input !== null ? (input as Record<string, unknown>) : {};
  const readString = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "string" && value.trim().length > 0) {
        return value.trim();
      }
    }
    return null;
  };
  const readBool = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "boolean") {
        return value;
      }
    }
    return null;
  };
  const readNumber = (...keys: string[]) => {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "number" && Number.isFinite(value)) {
        return value;
      }
      if (typeof value === "string") {
        const parsed = Number(value);
        if (Number.isFinite(parsed)) {
          return parsed;
        }
      }
    }
    return null;
  };

  const rawMode = (
    readString("mode", "notion_sync_mode", "sync_mode")?.toLowerCase() ?? "unknown"
  ) as SyncConfigSnapshot["mode"];
  const mode = rawMode === "page_tree" || rawMode === "database" ? rawMode : "unknown";

  return {
    mode,
    token_configured: readBool("token_configured", "notion_token_configured", "has_notion_token"),
    database_id_configured: readBool(
      "database_id_configured",
      "notion_database_id_configured",
      "has_notion_database_id"
    ),
    root_page_id_configured: readBool(
      "root_page_id_configured",
      "notion_root_page_id_configured",
      "has_notion_root_page_id"
    ),
    timezone: readString("timezone", "notion_timezone"),
    category_growth_limit: readNumber("category_growth_limit", "notion_category_growth_limit"),
    source
  };
}

export async function getSyncConfigSnapshot(): Promise<SyncConfigSnapshot> {
  if (!isTauriRuntime()) {
    return normalizeSyncConfigSnapshot({}, "fallback");
  }

  try {
    const data = await invokeWithEnvelope<unknown>("get_notion_sync_config_snapshot");
    return normalizeSyncConfigSnapshot(data, "ipc");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const unknownCommand =
      message.includes("unknown command") ||
      message.includes("not managed") ||
      message.includes("command get_notion_sync_config_snapshot not found");
    if (unknownCommand) {
      try {
        // Compatibility with older runtime naming.
        const legacy = await invokeWithEnvelope<unknown>("get_sync_config_snapshot");
        return normalizeSyncConfigSnapshot(legacy, "ipc");
      } catch (legacyError) {
        const legacyMessage = legacyError instanceof Error ? legacyError.message : String(legacyError);
        if (
          legacyMessage.includes("unknown command") ||
          legacyMessage.includes("not managed") ||
          legacyMessage.includes("command get_sync_config_snapshot not found")
        ) {
          return normalizeSyncConfigSnapshot({}, "fallback");
        }
        throw legacyError;
      }
    }
    throw error;
  }
}

const LAST_WECHAT_PICKER_DIR_KEY = "helper.import.lastPickerDir";

function normalizeDirFromPath(path: string): string | null {
  const normalized = path.trim();
  if (!normalized) {
    return null;
  }
  const separatorIndex = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  if (separatorIndex <= 0) {
    return null;
  }
  return normalized.slice(0, separatorIndex);
}

function readLastWechatPickerDir(): string | null {
  try {
    const raw = localStorage.getItem(LAST_WECHAT_PICKER_DIR_KEY);
    if (!raw) {
      return null;
    }
    const value = raw.trim();
    return value.length > 0 ? value : null;
  } catch {
    return null;
  }
}

function rememberWechatPickerDir(paths: string[]): void {
  try {
    const first = paths[0];
    if (!first) {
      return;
    }
    const dir = normalizeDirFromPath(first);
    if (!dir) {
      return;
    }
    localStorage.setItem(LAST_WECHAT_PICKER_DIR_KEY, dir);
  } catch {
    // Ignore localStorage errors, picker still works without remembering last directory.
  }
}

async function resolveWechatPickerDefaultPath(): Promise<string | undefined> {
  const lastDir = readLastWechatPickerDir();
  if (lastDir) {
    return lastDir;
  }
  try {
    const { homeDir } = await import("@tauri-apps/api/path");
    const home = await homeDir();
    const base = home.replace(/[\\/]$/, "");
    return `${base}/Library/Containers/com.tencent.xinWeChat/Data/Documents/xwechat_files`;
  } catch {
    return undefined;
  }
}

export { IpcInvokeError };
