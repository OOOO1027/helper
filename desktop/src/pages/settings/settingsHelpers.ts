import type { SyncConfigSnapshot, SyncRunSummary } from "../../types/contracts";
import { nowLocalIso } from "../../utils/time";

export type RangeDays = 1 | 3 | 7 | 30;
export type LogStateFilter = "all" | "started" | "success" | "failed" | "partial" | "skipped";
export type DailyJobTypeFilter = "all" | "notion_sync_once" | "notion_smoke";
export type ActionKind = "sync" | "replay" | null;

export const SETTINGS_FILTERS_KEY = "helper.settings.logs.filters";
export const LAST_SYNC_SUMMARY_KEY = "helper.sync.lastSummary";
export const IMPORT_HISTORY_KEY = "helper.import.history";
export const BILI_ENABLED_KEY = "helper.settings.bili.enabled";

export interface SettingsFilterSnapshot {
  rangeDays: RangeDays;
  logsStateFilter: LogStateFilter;
  dailyJobTypeFilter: DailyJobTypeFilter;
  dailyStatusFilter: LogStateFilter;
}

export interface LastSyncSnapshot {
  at: string;
  summary: SyncRunSummary;
}

export interface LatestImportSnapshot {
  at: string;
  status: "success" | "partial" | "failed";
  parsedOk: number;
  persisted: number;
  failed: number;
}

export interface ActionFeedback {
  level: "success" | "warning" | "error";
  message: string;
  nextStep?: string;
}

export function parseDeadLetterInput(input: string): string[] {
  const values = input
    .split(/[\s,，]+/)
    .map((item) => item.trim())
    .filter(Boolean);
  return [...new Set(values)];
}

function isRecord(input: unknown): input is Record<string, unknown> {
  return typeof input === "object" && input !== null;
}

function asNumber(input: unknown): number {
  if (typeof input === "number" && Number.isFinite(input)) {
    return input;
  }
  if (typeof input === "string") {
    const parsed = Number(input);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return 0;
}

export function loadFilterSnapshot(): SettingsFilterSnapshot {
  try {
    const raw = localStorage.getItem(SETTINGS_FILTERS_KEY);
    if (!raw) {
      return {
        rangeDays: 7,
        logsStateFilter: "failed",
        dailyJobTypeFilter: "all",
        dailyStatusFilter: "all",
      };
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed)) {
      return {
        rangeDays: 7,
        logsStateFilter: "failed",
        dailyJobTypeFilter: "all",
        dailyStatusFilter: "all",
      };
    }

    const rangeDays =
      parsed.rangeDays === 1 ||
      parsed.rangeDays === 3 ||
      parsed.rangeDays === 7 ||
      parsed.rangeDays === 30
        ? parsed.rangeDays
        : 7;
    const logsStateFilter =
      parsed.logsStateFilter === "all" ||
      parsed.logsStateFilter === "started" ||
      parsed.logsStateFilter === "success" ||
      parsed.logsStateFilter === "failed" ||
      parsed.logsStateFilter === "partial" ||
      parsed.logsStateFilter === "skipped"
        ? parsed.logsStateFilter
        : "failed";
    const dailyJobTypeFilter =
      parsed.dailyJobTypeFilter === "all" ||
      parsed.dailyJobTypeFilter === "notion_sync_once" ||
      parsed.dailyJobTypeFilter === "notion_smoke"
        ? parsed.dailyJobTypeFilter
        : "all";
    const dailyStatusFilter =
      parsed.dailyStatusFilter === "all" ||
      parsed.dailyStatusFilter === "started" ||
      parsed.dailyStatusFilter === "success" ||
      parsed.dailyStatusFilter === "failed" ||
      parsed.dailyStatusFilter === "partial" ||
      parsed.dailyStatusFilter === "skipped"
        ? parsed.dailyStatusFilter
        : "all";

    return {
      rangeDays,
      logsStateFilter,
      dailyJobTypeFilter,
      dailyStatusFilter,
    };
  } catch {
    return {
      rangeDays: 7,
      logsStateFilter: "failed",
      dailyJobTypeFilter: "all",
      dailyStatusFilter: "all",
    };
  }
}

export function loadLastSyncSnapshot(): LastSyncSnapshot | null {
  try {
    const raw = localStorage.getItem(LAST_SYNC_SUMMARY_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!isRecord(parsed) || !isRecord(parsed.summary) || typeof parsed.at !== "string") {
      return null;
    }
    const summary = parsed.summary;
    return {
      at: parsed.at,
      summary: {
        scanned: asNumber(summary.scanned),
        attempted: asNumber(summary.attempted),
        succeeded: asNumber(summary.succeeded),
        failed: asNumber(summary.failed),
        requeued: asNumber(summary.requeued),
        dead_lettered: asNumber(summary.dead_lettered),
      },
    };
  } catch {
    return null;
  }
}

export function loadLatestImportSnapshot(): LatestImportSnapshot | null {
  try {
    const raw = localStorage.getItem(IMPORT_HISTORY_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed) || parsed.length === 0) {
      return null;
    }
    const latest = parsed[0];
    if (!isRecord(latest) || !isRecord(latest.summary)) {
      return null;
    }
    const summary = latest.summary;
    const failed = asNumber(summary.parse_failed) + asNumber(summary.persist_failed);
    return {
      at: typeof latest.createdAt === "string" ? latest.createdAt : nowLocalIso(),
      status: failed === 0 ? "success" : asNumber(summary.persisted) > 0 ? "partial" : "failed",
      parsedOk: asNumber(summary.parsed_ok),
      persisted: asNumber(summary.persisted),
      failed,
    };
  } catch {
    return null;
  }
}

export function loadBiliEnabled(): boolean {
  try {
    const raw = localStorage.getItem(BILI_ENABLED_KEY);
    if (!raw) {
      return false;
    }
    const parsed = JSON.parse(raw) as unknown;
    return parsed === true;
  } catch {
    return false;
  }
}

export function formatJobType(jobType: string): string {
  if (jobType === "notion_sync_once") {
    return "Notion 单次同步";
  }
  if (jobType === "dead_letter_replay") {
    return "失败回放";
  }
  return jobType;
}

export function formatLogState(state: string): string {
  if (state === "started") {
    return "进行中";
  }
  if (state === "success") {
    return "成功";
  }
  if (state === "failed") {
    return "失败";
  }
  if (state === "partial") {
    return "部分成功";
  }
  if (state === "skipped") {
    return "已跳过";
  }
  return state;
}

export function logStateTone(state: string): "success" | "warning" | "error" | "neutral" {
  if (state === "success") {
    return "success";
  }
  if (state === "failed") {
    return "error";
  }
  if (state === "partial" || state === "started") {
    return "warning";
  }
  return "neutral";
}

export function modeLabel(mode: SyncConfigSnapshot["mode"]): string {
  if (mode === "database") {
    return "database";
  }
  if (mode === "page_tree") {
    return "page_tree";
  }
  return "未知";
}

export function statusLabel(value: boolean | null): string {
  if (value === true) {
    return "已配置";
  }
  if (value === false) {
    return "未配置";
  }
  return "未知";
}
