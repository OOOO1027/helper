import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { NoticeItem } from "../App";
import { RemoteState } from "../components/RemoteState";
import {
  getSyncConfigSnapshot,
  getSyncDailyStats,
  getSyncLogs,
  retryDeadLetters,
  runNotionSyncOnce,
  toUserMessage,
} from "../services/ipc";
import type {
  SyncConfigSnapshot,
  SyncDailyStat,
  SyncLog,
  SyncRunSummary,
} from "../types/contracts";
import { formatTime, nowLocalIso } from "../utils/time";

interface Props {
  onNotify: (item: Omit<NoticeItem, "id">) => void;
}

type RangeDays = 1 | 3 | 7 | 30;
type LogStateFilter = "all" | "started" | "success" | "failed" | "partial" | "skipped";
type DailyJobTypeFilter = "all" | "notion_sync_once" | "notion_smoke";
type ActionKind = "sync" | "replay" | null;

const SETTINGS_FILTERS_KEY = "helper.settings.logs.filters";
const LAST_SYNC_SUMMARY_KEY = "helper.sync.lastSummary";
const IMPORT_HISTORY_KEY = "helper.import.history";
const BILI_ENABLED_KEY = "helper.settings.bili.enabled";

interface SettingsFilterSnapshot {
  rangeDays: RangeDays;
  logsStateFilter: LogStateFilter;
  dailyJobTypeFilter: DailyJobTypeFilter;
  dailyStatusFilter: LogStateFilter;
}

interface LastSyncSnapshot {
  at: string;
  summary: SyncRunSummary;
}

interface LatestImportSnapshot {
  at: string;
  status: "success" | "partial" | "failed";
  parsedOk: number;
  persisted: number;
  failed: number;
}

interface ActionFeedback {
  level: "success" | "warning" | "error";
  message: string;
  nextStep?: string;
}

function parseDeadLetterInput(input: string): string[] {
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

function loadFilterSnapshot(): SettingsFilterSnapshot {
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

function loadLastSyncSnapshot(): LastSyncSnapshot | null {
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

function loadLatestImportSnapshot(): LatestImportSnapshot | null {
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

function loadBiliEnabled(): boolean {
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

function formatJobType(jobType: string): string {
  if (jobType === "notion_sync_once") {
    return "Notion 单次同步";
  }
  if (jobType === "dead_letter_replay") {
    return "失败回放";
  }
  return jobType;
}

function formatLogState(state: string): string {
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

function logStateTone(state: string): "success" | "warning" | "error" | "neutral" {
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

function modeLabel(mode: SyncConfigSnapshot["mode"]): string {
  if (mode === "database") {
    return "database";
  }
  if (mode === "page_tree") {
    return "page_tree";
  }
  return "未知";
}

function statusLabel(value: boolean | null): string {
  if (value === true) {
    return "已配置";
  }
  if (value === false) {
    return "未配置";
  }
  return "未知";
}

export function SettingsLogsPage({ onNotify }: Props) {
  const persistedFilters = useMemo(() => loadFilterSnapshot(), []);
  const logsPageSize = 20;
  const statsPageSize = 20;
  const [biliEnabled, setBiliEnabled] = useState<boolean>(() => loadBiliEnabled());
  const [logs, setLogs] = useState<SyncLog[]>([]);
  const [logsTotal, setLogsTotal] = useState(0);
  const [logsPage, setLogsPage] = useState(1);
  const [dailyStats, setDailyStats] = useState<SyncDailyStat[]>([]);
  const [statsTotal, setStatsTotal] = useState(0);
  const [statsPage, setStatsPage] = useState(1);
  const [logsLoading, setLogsLoading] = useState(true);
  const [statsLoading, setStatsLoading] = useState(true);
  const [logsError, setLogsError] = useState<string | null>(null);
  const [statsError, setStatsError] = useState<string | null>(null);
  const [syncConfig, setSyncConfig] = useState<SyncConfigSnapshot | null>(null);
  const [syncConfigLoading, setSyncConfigLoading] = useState(true);
  const [syncConfigError, setSyncConfigError] = useState<string | null>(null);
  const [rangeDays, setRangeDays] = useState<RangeDays>(persistedFilters.rangeDays);
  const [logsStateFilter, setLogsStateFilter] = useState<LogStateFilter>(
    persistedFilters.logsStateFilter
  );
  const [dailyJobTypeFilter, setDailyJobTypeFilter] = useState<DailyJobTypeFilter>(
    persistedFilters.dailyJobTypeFilter
  );
  const [dailyStatusFilter, setDailyStatusFilter] = useState<LogStateFilter>(
    persistedFilters.dailyStatusFilter
  );
  const [dailyStatusCustomInput, setDailyStatusCustomInput] = useState("");
  const [dailyStatusOverride, setDailyStatusOverride] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncLimitInput, setSyncLimitInput] = useState("50");
  const [lastSyncSummary, setLastSyncSummary] = useState<SyncRunSummary | null>(null);
  const [lastSyncSnapshot, setLastSyncSnapshot] = useState<LastSyncSnapshot | null>(() =>
    loadLastSyncSnapshot()
  );
  const [latestImportSnapshot] = useState<LatestImportSnapshot | null>(() =>
    loadLatestImportSnapshot()
  );
  const [actionFeedback, setActionFeedback] = useState<ActionFeedback | null>(null);
  const [lastActionKind, setLastActionKind] = useState<ActionKind>(null);
  const [deadLetterIds, setDeadLetterIds] = useState("");
  const [retryingDeadLetters, setRetryingDeadLetters] = useState(false);
  const actionBusy = syncing || retryingDeadLetters;
  const hasActiveFilters =
    rangeDays !== 7 ||
    logsStateFilter !== "failed" ||
    dailyJobTypeFilter !== "all" ||
    dailyStatusFilter !== "all" ||
    dailyStatusOverride !== null;
  const requestSeqRef = useRef(0);
  const logsTotalPages = Math.max(1, Math.ceil(logsTotal / logsPageSize));
  const statsTotalPages = Math.max(1, Math.ceil(statsTotal / statsPageSize));
  const rangeHint = `最近 ${rangeDays} 天`;
  const filteredLogs = useMemo(() => {
    if (logsStateFilter === "all") {
      return logs;
    }
    return logs.filter((item) => item.state === logsStateFilter);
  }, [logs, logsStateFilter]);
  const logStateStats = useMemo(() => {
    const counter = new Map<string, number>();
    for (const item of logs) {
      counter.set(item.state, (counter.get(item.state) ?? 0) + 1);
    }
    return [...counter.entries()].sort((a, b) => b[1] - a[1]);
  }, [logs]);
  const failedCount = useMemo(
    () => logs.filter((item) => item.state === "failed" || item.state === "partial").length,
    [logs]
  );
  const replayableCandidateCount = useMemo(
    () =>
      logs.filter(
        (item) =>
          (item.state === "failed" || item.state === "partial") &&
          typeof item.error_code === "string" &&
          item.error_code.length > 0
      ).length,
    [logs]
  );
  const deadLetterIdCount = useMemo(
    () => parseDeadLetterInput(deadLetterIds).length,
    [deadLetterIds]
  );
  const latestSyncHeadline = useMemo(() => {
    if (lastSyncSummary) {
      const failed = lastSyncSummary.failed;
      if (failed === 0) {
        return {
          status: "success" as const,
          text: `成功 ${lastSyncSummary.succeeded} / 失败 0`,
          at: nowLocalIso(),
        };
      }
      return {
        status: "warning" as const,
        text: `成功 ${lastSyncSummary.succeeded} / 失败 ${failed}`,
        at: nowLocalIso(),
      };
    }
    if (lastSyncSnapshot) {
      const failed = lastSyncSnapshot.summary.failed;
      return {
        status: failed === 0 ? ("success" as const) : ("warning" as const),
        text: `成功 ${lastSyncSnapshot.summary.succeeded} / 失败 ${failed}`,
        at: lastSyncSnapshot.at,
      };
    }
    const latestLog = logs[0];
    if (!latestLog) {
      return null;
    }
    return {
      status:
        latestLog.state === "success"
          ? ("success" as const)
          : latestLog.state === "failed" || latestLog.state === "partial"
            ? ("warning" as const)
            : ("neutral" as const),
      text: `最近状态：${formatLogState(latestLog.state)}`,
      at: latestLog.created_at,
    };
  }, [lastSyncSnapshot, lastSyncSummary, logs]);
  const dailyOverview = useMemo(() => {
    if (dailyStats.length === 0) {
      return null;
    }
    let totalRuns = 0;
    let totalSuccessRuns = 0;
    let weightedAvgSuccessCount = 0;
    let weightedAvgFailCount = 0;
    const daySet = new Set<string>();
    const jobTypeCounter = new Map<string, number>();

    for (const item of dailyStats) {
      totalRuns += item.run_count;
      totalSuccessRuns += item.success_runs;
      weightedAvgSuccessCount += item.avg_success_count * item.run_count;
      weightedAvgFailCount += item.avg_fail_count * item.run_count;
      daySet.add(item.day);
      jobTypeCounter.set(item.job_type, (jobTypeCounter.get(item.job_type) ?? 0) + item.run_count);
    }

    const topJobTypeEntry = [...jobTypeCounter.entries()].sort((a, b) => b[1] - a[1])[0];
    return {
      totalRuns,
      successRate: totalRuns > 0 ? totalSuccessRuns / totalRuns : 0,
      avgSuccessPerRun: totalRuns > 0 ? weightedAvgSuccessCount / totalRuns : 0,
      avgFailPerRun: totalRuns > 0 ? weightedAvgFailCount / totalRuns : 0,
      dayCount: daySet.size,
      jobTypeCount: jobTypeCounter.size,
      topJobType: topJobTypeEntry ? formatJobType(topJobTypeEntry[0]) : "-",
    };
  }, [dailyStats]);
  const syncMode = syncConfig?.mode ?? "unknown";
  const syncModeUnknown = syncMode === "unknown";
  const rootPageMissing = syncMode === "page_tree" && syncConfig?.root_page_id_configured === false;
  const databaseIdMissing = syncMode === "database" && syncConfig?.database_id_configured === false;
  const tokenMissing = syncConfig?.token_configured === false;

  const notifyAction = (level: "success" | "warning" | "error", action: string, detail: string) => {
    onNotify({ level, message: `${action}：${detail}` });
  };

  const loadSyncConfig = useCallback(async () => {
    setSyncConfigLoading(true);
    try {
      const snapshot = await getSyncConfigSnapshot();
      setSyncConfig(snapshot);
      setSyncConfigError(null);
      return snapshot;
    } catch (error) {
      setSyncConfig(null);
      setSyncConfigError(`同步配置加载失败：${toUserMessage(error, "获取同步配置失败")}`);
      return null;
    } finally {
      setSyncConfigLoading(false);
    }
  }, []);

  const loadSyncOverview = useCallback(async () => {
    const requestSeq = requestSeqRef.current + 1;
    requestSeqRef.current = requestSeq;
    const to = nowLocalIso();
    const fromDate = new Date();
    fromDate.setDate(fromDate.getDate() - rangeDays);
    const range = { from: fromDate.toISOString(), to };
    setLogsLoading(true);
    setStatsLoading(true);
    const [logsResult, dailyStatsResult] = await Promise.allSettled([
      getSyncLogs(range, { page: logsPage, page_size: logsPageSize }),
      getSyncDailyStats(
        range,
        { page: statsPage, page_size: statsPageSize },
        {
          job_type: dailyJobTypeFilter === "all" ? undefined : dailyJobTypeFilter,
          status:
            dailyStatusOverride && dailyStatusOverride.trim().length > 0
              ? dailyStatusOverride.trim()
              : dailyStatusFilter === "all"
                ? undefined
                : dailyStatusFilter,
        }
      ),
    ]);

    if (requestSeq !== requestSeqRef.current) {
      return;
    }

    if (logsResult.status === "fulfilled") {
      setLogs(logsResult.value.items);
      setLogsTotal(logsResult.value.total);
      setLogsError(null);
    } else {
      setLogsError(`同步数据加载失败：${toUserMessage(logsResult.reason, "获取日志失败")}`);
    }
    setLogsLoading(false);

    if (dailyStatsResult.status === "fulfilled") {
      setDailyStats(dailyStatsResult.value.items);
      setStatsTotal(dailyStatsResult.value.total);
      setStatsError(null);
    } else {
      setStatsError(
        `同步数据加载失败：${toUserMessage(dailyStatsResult.reason, "获取日统计失败")}`
      );
    }
    setStatsLoading(false);
  }, [
    dailyJobTypeFilter,
    dailyStatusFilter,
    dailyStatusOverride,
    logsPage,
    logsPageSize,
    rangeDays,
    statsPage,
    statsPageSize,
  ]);

  useEffect(() => {
    let cancelled = false;
    loadSyncOverview().finally(() => {
      if (!cancelled) {
        setLogsLoading(false);
        setStatsLoading(false);
      }
    });

    return () => {
      cancelled = true;
      requestSeqRef.current += 1;
    };
  }, [loadSyncOverview]);

  useEffect(() => {
    void loadSyncConfig();
  }, [loadSyncConfig]);

  useEffect(() => {
    const snapshot: SettingsFilterSnapshot = {
      rangeDays,
      logsStateFilter,
      dailyJobTypeFilter,
      dailyStatusFilter,
    };
    localStorage.setItem(SETTINGS_FILTERS_KEY, JSON.stringify(snapshot));
  }, [dailyJobTypeFilter, dailyStatusFilter, logsStateFilter, rangeDays]);

  useEffect(() => {
    localStorage.setItem(BILI_ENABLED_KEY, JSON.stringify(biliEnabled));
  }, [biliEnabled]);

  const onManualSync = async () => {
    setLastActionKind("sync");
    setActionFeedback(null);
    const currentConfig = syncConfig ?? (await loadSyncConfig());
    if (!currentConfig) {
      setActionFeedback({
        level: "warning",
        message: "未获取到同步配置状态，已拦截本次同步。",
        nextStep: "下一步：先点击“刷新配置状态”确认配置，再执行“立即同步一次”。",
      });
      notifyAction("warning", "立即同步", "同步配置未就绪，已阻止请求");
      return;
    }
    if (currentConfig.source === "ipc" && currentConfig.mode === "unknown") {
      setActionFeedback({
        level: "warning",
        message: "同步模式配置无效，已拦截本次同步。",
        nextStep: "下一步：将 NOTION_SYNC_MODE 设置为 page_tree 或 database 后重试。",
      });
      notifyAction("warning", "立即同步", "同步模式配置无效");
      return;
    }
    if (currentConfig.mode === "page_tree" && currentConfig.root_page_id_configured === false) {
      setActionFeedback({
        level: "warning",
        message: "当前是页面树模式，但未配置根页面，已拦截本次同步。",
        nextStep: "下一步：在部署配置中补齐根页面 ID，再回到这里重试。",
      });
      notifyAction("warning", "立即同步", "页面树模式缺少根页面配置");
      return;
    }
    if (currentConfig.mode === "database" && currentConfig.database_id_configured === false) {
      setActionFeedback({
        level: "warning",
        message: "当前是数据库模式，但未配置数据库 ID，已拦截本次同步。",
        nextStep: "下一步：在部署配置中补齐数据库 ID，再回到这里重试。",
      });
      notifyAction("warning", "立即同步", "数据库模式缺少数据库 ID 配置");
      return;
    }
    if (currentConfig.token_configured === false) {
      setActionFeedback({
        level: "warning",
        message: "未检测到 Notion 凭证，已拦截本次同步。",
        nextStep: "下一步：先配置 Notion 访问凭证，再执行“立即同步一次”。",
      });
      notifyAction("warning", "立即同步", "缺少 Notion 凭证");
      return;
    }

    const normalized = syncLimitInput.trim();
    const parsedLimit = normalized.length === 0 ? undefined : Number(normalized);
    if (parsedLimit !== undefined && (!Number.isFinite(parsedLimit) || parsedLimit <= 0)) {
      setActionFeedback({
        level: "warning",
        message: "同步上限无效，请输入大于 0 的数字。",
        nextStep: "下一步：修正同步上限后，重新点击“立即同步一次”。",
      });
      notifyAction("warning", "立即同步", "请输入大于 0 的处理上限");
      return;
    }

    setSyncing(true);
    try {
      const summary = await runNotionSyncOnce(
        parsedLimit === undefined ? undefined : Math.floor(parsedLimit)
      );
      setLastSyncSummary(summary);
      const snapshot: LastSyncSnapshot = {
        at: nowLocalIso(),
        summary,
      };
      setLastSyncSnapshot(snapshot);
      localStorage.setItem(LAST_SYNC_SUMMARY_KEY, JSON.stringify(snapshot));
      const level = summary.failed > 0 ? "warning" : "success";
      setActionFeedback({
        level,
        message: `同步完成：成功 ${summary.succeeded} / 失败 ${summary.failed}`,
        nextStep:
          summary.failed > 0
            ? "下一步：点击“查看失败日志”定位失败项，必要时执行失败回放。"
            : "下一步：前往日志确认最新记录，继续下一轮导入或审核。",
      });
      notifyAction(level, "立即同步", `成功 ${summary.succeeded} / 失败 ${summary.failed}`);
    } catch (e) {
      const message = toUserMessage(e, "Notion 同步失败");
      setActionFeedback({
        level: "error",
        message,
        nextStep: "下一步：检查 Notion 连接与本地存储后重试；必要时在日志中筛选失败项。",
      });
      notifyAction("error", "立即同步", message);
    } finally {
      await Promise.all([loadSyncOverview(), loadSyncConfig()]);
      setSyncing(false);
    }
  };

  const onRetryDeadLetters = async () => {
    setLastActionKind("replay");
    setActionFeedback(null);
    const ids = parseDeadLetterInput(deadLetterIds);
    if (ids.length === 0) {
      setActionFeedback({
        level: "warning",
        message: "未输入可回放 ID。",
        nextStep: "下一步：先在失败日志中复制 ID，再回到这里执行回放。",
      });
      notifyAction("warning", "回放 dead letters", "请先输入需要回放的失败项 ID");
      return;
    }

    setRetryingDeadLetters(true);
    try {
      const result = await retryDeadLetters(ids);
      const level = result.requeued === result.requested ? "success" : "warning";
      notifyAction(
        level,
        "回放 dead letters",
        `重排 ${result.requeued}/${result.requested}，忽略 ${result.ignored}`
      );
      setActionFeedback({
        level,
        message: `回放完成：重排 ${result.requeued}/${result.requested}，忽略 ${result.ignored}`,
        nextStep:
          result.requeued === result.requested
            ? "下一步：点击“刷新日志”确认状态变化。"
            : "下一步：检查输入 ID 是否来自失败日志，再次回放未成功项。",
      });
      setDeadLetterIds("");
    } catch (e) {
      const message = toUserMessage(e, "重排 dead letters 失败");
      setActionFeedback({
        level: "error",
        message,
        nextStep: "下一步：确认失败项 ID 来自日志失败记录，修正后再回放。",
      });
      notifyAction("error", "回放 dead letters", message);
    } finally {
      await Promise.all([loadSyncOverview(), loadSyncConfig()]);
      setRetryingDeadLetters(false);
    }
  };

  const onQueueReplayId = (id: string) => {
    setDeadLetterIds((prev) => {
      const current = parseDeadLetterInput(prev);
      if (current.includes(id)) {
        return prev;
      }
      return [...current, id].join(", ");
    });
    onNotify({ level: "info", message: `已加入回放队列：${id.slice(0, 8)}` });
  };

  const onApplyCustomDailyStatus = () => {
    const normalized = dailyStatusCustomInput.trim();
    setStatsPage(1);
    if (!normalized) {
      setDailyStatusOverride(null);
      onNotify({ level: "info", message: "已清除自定义运行状态筛选，恢复下拉筛选。" });
      return;
    }
    setDailyStatusOverride(normalized);
    onNotify({ level: "info", message: `已应用自定义运行状态筛选：${normalized}` });
  };

  return (
    <section>
      <header className="page-header">
        <h1>设置与日志</h1>
        <p>Notion 仅展示轻提示，不暴露复杂技术细节。</p>
      </header>

      <section className="card-grid settings-acceptance">
        <article className="metric-card">
          <span>最近一次导入</span>
          {latestImportSnapshot ? (
            <>
              <strong>
                {latestImportSnapshot.status === "success"
                  ? "成功"
                  : latestImportSnapshot.status === "partial"
                    ? "部分成功"
                    : "失败"}
              </strong>
              <small>
                解析 {latestImportSnapshot.parsedOk} · 入库 {latestImportSnapshot.persisted} · 失败{" "}
                {latestImportSnapshot.failed}
              </small>
              <small>{formatTime(latestImportSnapshot.at)}</small>
            </>
          ) : (
            <>
              <strong>暂无记录</strong>
              <small>先在导入中心完成一次导入</small>
            </>
          )}
        </article>
        <article className="metric-card">
          <span>最近一次同步</span>
          {latestSyncHeadline ? (
            <>
              <strong>{latestSyncHeadline.text}</strong>
              <small>{formatTime(latestSyncHeadline.at)}</small>
            </>
          ) : (
            <>
              <strong>暂无记录</strong>
              <small>点击“立即同步一次”开始同步</small>
            </>
          )}
        </article>
        <article className="metric-card">
          <span>失败数量（当前页）</span>
          <strong>{failedCount}</strong>
          <small>可通过状态筛选快速定位失败项</small>
        </article>
        <article className="metric-card">
          <span>可回放候选</span>
          <strong>{replayableCandidateCount}</strong>
          <small>从失败日志复制 ID 后可直接回放</small>
        </article>
      </section>

      {dailyOverview && (
        <section className="card-grid settings-metrics">
          <article className="metric-card">
            <span>7天运行总数</span>
            <strong>{dailyOverview.totalRuns}</strong>
            <small>
              覆盖 {dailyOverview.dayCount} 天 · {dailyOverview.jobTypeCount} 类任务
            </small>
          </article>
          <article className="metric-card">
            <span>整体成功率</span>
            <strong>{`${Math.round(dailyOverview.successRate * 100)}%`}</strong>
            <div className="metric-meter">
              <div
                className={`metric-meter-fill ${dailyOverview.successRate < 0.8 ? "warn" : ""}`}
                style={{ width: `${Math.round(dailyOverview.successRate * 100)}%` }}
              />
            </div>
          </article>
          <article className="metric-card">
            <span>单次平均成功</span>
            <strong>{dailyOverview.avgSuccessPerRun.toFixed(2)}</strong>
            <small>用于衡量同步产出稳定性</small>
          </article>
          <article className="metric-card">
            <span>高频任务类型</span>
            <strong>{dailyOverview.topJobType}</strong>
            <small>7天内触发次数最高</small>
          </article>
        </section>
      )}

      <article className="panel-card">
        <h3>同步配置状态</h3>
        <p className="hint">用于预判“立即同步一次”是否可执行，避免点了才失败。</p>
        <RemoteState
          loading={syncConfigLoading}
          error={syncConfigError}
          empty={!syncConfig}
          loadingText="同步配置加载中..."
          emptyText="暂未获取到同步配置快照。"
          errorNextStep="下一步：点击“刷新配置状态”；若持续失败，请确认后端配置快照 IPC 已部署。"
          emptyNextStep="下一步：点击“刷新配置状态”，确认模式与配置项后再执行同步。"
          onRetry={() => void loadSyncConfig()}
          retryLabel="刷新配置状态"
        >
          <div className="kv-row">
            <span>当前模式</span>
            <code>{modeLabel(syncConfig?.mode ?? "unknown")}</code>
          </div>
          <div className="kv-row">
            <span>Notion 凭证</span>
            <code>{statusLabel(syncConfig?.token_configured ?? null)}</code>
          </div>
          <div className="kv-row">
            <span>database id</span>
            <code>{statusLabel(syncConfig?.database_id_configured ?? null)}</code>
          </div>
          <div className="kv-row">
            <span>root page id</span>
            <code>{statusLabel(syncConfig?.root_page_id_configured ?? null)}</code>
          </div>
          <div className="kv-row">
            <span>timezone</span>
            <code>
              {syncConfig?.timezone && syncConfig.timezone.length > 0 ? syncConfig.timezone : "-"}
            </code>
          </div>
          <div className="kv-row">
            <span>category growth limit</span>
            <code>{syncConfig?.category_growth_limit ?? "-"}</code>
          </div>
          {syncConfig?.source === "fallback" && (
            <div className="state-panel warning" role="status" aria-live="polite">
              当前运行时尚未返回配置快照，前置校验会按最小可用策略执行。
              <small>下一步：建议升级后端到包含配置快照 IPC 的版本，获得完整预检能力。</small>
            </div>
          )}
          {syncConfig?.source === "ipc" && syncModeUnknown && (
            <div className="state-panel warning" role="status" aria-live="polite">
              当前同步模式配置无效，手动同步会被拦截。
              <small>
                下一步：将 NOTION_SYNC_MODE 设为 page_tree 或 database，再点击“刷新配置状态”。
              </small>
            </div>
          )}
          {rootPageMissing && (
            <div className="state-panel warning" role="status" aria-live="polite">
              当前为页面树模式，但未配置“同步起始页面”，本次同步会被拦截。
              <small>下一步：在环境配置中补齐 root page id，再回到此页重试。</small>
            </div>
          )}
          {databaseIdMissing && (
            <div className="state-panel warning" role="status" aria-live="polite">
              当前为数据库模式，但未配置“目标数据库”，本次同步会被拦截。
              <small>下一步：在环境配置中补齐 database id，再回到此页重试。</small>
            </div>
          )}
          {tokenMissing && (
            <div className="state-panel warning" role="status" aria-live="polite">
              当前未配置 Notion 凭证，本次同步会被拦截。
              <small>下一步：先补齐凭证，再执行手动同步或失败回放。</small>
            </div>
          )}
          <div className="inline-actions">
            <button
              type="button"
              onClick={() => void loadSyncConfig()}
              disabled={actionBusy || syncConfigLoading}
            >
              {syncConfigLoading ? "刷新中..." : "刷新配置状态"}
            </button>
          </div>
        </RemoteState>
      </article>

      <div className="settings-grid">
        <article className="panel-card">
          <h3>来源设置</h3>
          <label className="switch-row">
            <span>B站自动抓取</span>
            <input
              type="checkbox"
              checked={biliEnabled}
              onChange={(e) => setBiliEnabled(e.target.checked)}
            />
            <small>{biliEnabled ? "已开启" : "默认关闭"}</small>
          </label>
          <p className="hint">当前默认关闭，避免无意产生额外采集负载。该开关仅在本机生效。</p>
        </article>

        <article className="panel-card">
          <h3>Notion 同步提示</h3>
          <p>展示层状态：已连接/可同步/失败可重试。</p>
          <p className="hint">不展示 token、字段映射等技术细节。</p>
          <input
            className="text-input"
            type="number"
            min={1}
            step={1}
            value={syncLimitInput}
            onChange={(event) => setSyncLimitInput(event.target.value)}
            placeholder="同步上限（默认 50）"
            disabled={actionBusy}
          />
          <div className="inline-actions">
            <button
              type="button"
              className="button-primary"
              onClick={onManualSync}
              disabled={actionBusy}
              data-testid="settings-manual-sync"
            >
              {syncing ? "同步中..." : "立即同步一次"}
            </button>
            <span className="hint">本次仅执行一轮同步，不修改常驻策略。</span>
          </div>
          {actionBusy && (
            <div className="state-panel" role="status" aria-live="polite">
              {syncing
                ? "正在执行手动同步，请等待结果返回。"
                : "正在执行失败回放，请等待结果返回。"}
            </div>
          )}
          {lastSyncSummary && (
            <div className="state-panel">
              最近手动同步：扫描 {lastSyncSummary.scanned}，尝试 {lastSyncSummary.attempted}，成功{" "}
              {lastSyncSummary.succeeded}，失败 {lastSyncSummary.failed}。
            </div>
          )}
        </article>

        <article className="panel-card">
          <h3>失败回放</h3>
          <p className="hint">输入 dead letter 项目 ID（逗号或空格分隔），可手动回放入队。</p>
          <input
            className="text-input"
            value={deadLetterIds}
            onChange={(event) => setDeadLetterIds(event.target.value)}
            placeholder="例如：dl_123, dl_456"
            disabled={actionBusy}
            data-testid="settings-dead-letter-ids"
          />
          <div className="inline-actions">
            <button
              type="button"
              onClick={() => setDeadLetterIds("")}
              disabled={actionBusy || deadLetterIds.trim().length === 0}
              data-testid="settings-clear-dead-letter-ids"
            >
              清空 ID
            </button>
            <button
              type="button"
              className="button-primary"
              onClick={onRetryDeadLetters}
              disabled={actionBusy}
              data-testid="settings-retry-dead-letters"
            >
              {retryingDeadLetters ? "回放中..." : "回放失败项"}
            </button>
            <span className="hint">
              已填入 {deadLetterIdCount} 个 ID，不会覆盖历史记录，仅重排待处理队列。
            </span>
          </div>
        </article>
      </div>

      {actionFeedback && (
        <div
          className={`state-panel ${actionFeedback.level === "error" ? "error" : actionFeedback.level === "warning" ? "warning" : ""}`}
          role={actionFeedback.level === "error" ? "alert" : "status"}
          aria-live={actionFeedback.level === "error" ? "assertive" : "polite"}
          data-testid="settings-action-feedback"
        >
          <div>{actionFeedback.message}</div>
          {actionFeedback.nextStep && <small>{actionFeedback.nextStep}</small>}
          <div className="inline-actions">
            {lastActionKind === "sync" && (
              <button
                type="button"
                onClick={onManualSync}
                disabled={actionBusy}
                data-testid="settings-retry-sync"
              >
                重试同步
              </button>
            )}
            {lastActionKind === "replay" && (
              <button
                type="button"
                onClick={onRetryDeadLetters}
                disabled={actionBusy}
                data-testid="settings-retry-replay"
              >
                重试回放
              </button>
            )}
            <button
              type="button"
              onClick={() => {
                setLogsStateFilter("failed");
                setRangeDays(7);
                setLogsPage(1);
              }}
              disabled={actionBusy}
              data-testid="settings-view-failed-logs"
            >
              查看失败日志
            </button>
          </div>
        </div>
      )}

      <article className="panel-card">
        <h3>同步日志（{rangeHint}）</h3>
        <div className="filter-bar">
          <select
            value={rangeDays}
            onChange={(event) => {
              setRangeDays(Number(event.target.value) as 1 | 3 | 7 | 30);
              setLogsPage(1);
              setStatsPage(1);
            }}
            disabled={actionBusy}
          >
            <option value={1}>最近 1 天</option>
            <option value={3}>最近 3 天</option>
            <option value={7}>最近 7 天</option>
            <option value={30}>最近 30 天</option>
          </select>
          <select
            value={logsStateFilter}
            onChange={(event) =>
              setLogsStateFilter(
                event.target.value as
                  | "all"
                  | "started"
                  | "success"
                  | "failed"
                  | "partial"
                  | "skipped"
              )
            }
            disabled={actionBusy}
          >
            <option value="failed">状态：失败优先</option>
            <option value="all">状态：全部</option>
            <option value="started">进行中</option>
            <option value="success">成功</option>
            <option value="partial">部分成功</option>
            <option value="skipped">已跳过</option>
          </select>
          <span className="hint">时间范围切换后自动刷新日志与日统计</span>
          <button
            type="button"
            onClick={() => {
              setRangeDays(7);
              setLogsStateFilter("failed");
              setDailyJobTypeFilter("all");
              setDailyStatusFilter("all");
              setDailyStatusCustomInput("");
              setDailyStatusOverride(null);
              setLogsPage(1);
              setStatsPage(1);
            }}
            disabled={actionBusy || !hasActiveFilters}
          >
            重置筛选
          </button>
        </div>
        {logStateStats.length > 0 && (
          <div className="reason-bar">
            <button
              type="button"
              className={logsStateFilter === "all" ? "reason-chip active" : "reason-chip"}
              onClick={() => setLogsStateFilter("all")}
              disabled={actionBusy}
            >
              全部 ({logs.length})
            </button>
            {logStateStats.map(([state, count]) => (
              <button
                key={state}
                type="button"
                className={logsStateFilter === state ? "reason-chip active" : "reason-chip"}
                onClick={() =>
                  setLogsStateFilter(
                    state as "started" | "success" | "failed" | "partial" | "skipped"
                  )
                }
                disabled={actionBusy}
              >
                {formatLogState(state)} ({count})
              </button>
            ))}
          </div>
        )}
        <RemoteState
          loading={logsLoading}
          error={logsError}
          empty={filteredLogs.length === 0}
          loadingText="同步日志加载中..."
          emptyText="当前筛选条件下暂无同步日志。可放宽时间范围或切换状态后重试。"
          errorNextStep="下一步：点击“重试加载日志”；若仍失败，请先执行一次“立即同步一次”再刷新。"
          emptyNextStep="下一步：先将状态切到“全部”或扩大时间范围，再点击“刷新日志”。"
          onRetry={() => void loadSyncOverview()}
          retryLabel="重试加载日志"
        >
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>ID</th>
                  <th>状态</th>
                  <th>错误码</th>
                  <th>时间</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {filteredLogs.map((log) => (
                  <tr key={log.id}>
                    <td>{log.id.slice(0, 8)}</td>
                    <td>
                      <span className={`tag ${logStateTone(log.state)}`}>
                        {formatLogState(log.state)}
                      </span>
                    </td>
                    <td>{log.error_code ?? "-"}</td>
                    <td>{formatTime(log.created_at)}</td>
                    <td className="table-action-cell">
                      {(log.state === "failed" || log.state === "partial") && log.error_code ? (
                        <button
                          type="button"
                          className="table-action-button"
                          disabled={actionBusy}
                          onClick={() => onQueueReplayId(log.id)}
                          data-testid={`settings-queue-replay-${log.id.slice(0, 8)}`}
                        >
                          加入回放
                        </button>
                      ) : (
                        <span className="hint">-</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </RemoteState>
        <div className="inline-actions">
          <button
            type="button"
            onClick={() => setLogsPage((prev) => Math.max(prev - 1, 1))}
            disabled={actionBusy || logsLoading || logsPage <= 1}
          >
            上一页
          </button>
          <span className="hint">
            日志页 {logsPage}/{logsTotalPages} · 共 {logsTotal} 条
          </span>
          <button
            type="button"
            onClick={() => setLogsPage((prev) => Math.min(prev + 1, logsTotalPages))}
            disabled={actionBusy || logsLoading || logsPage >= logsTotalPages}
          >
            下一页
          </button>
          <button
            type="button"
            onClick={() => void loadSyncOverview()}
            disabled={actionBusy || logsLoading}
            data-testid="settings-refresh-logs"
          >
            刷新日志
          </button>
        </div>
      </article>

      <article className="panel-card">
        <h3>日统计（{rangeHint}）</h3>
        <div className="filter-bar">
          <select
            value={dailyJobTypeFilter}
            onChange={(event) => {
              setDailyJobTypeFilter(
                event.target.value as "all" | "notion_sync_once" | "notion_smoke"
              );
              setStatsPage(1);
            }}
            disabled={actionBusy}
          >
            <option value="all">任务类型：全部</option>
            <option value="notion_sync_once">notion_sync_once</option>
            <option value="notion_smoke">notion_smoke</option>
          </select>
          <select
            value={dailyStatusFilter}
            onChange={(event) => {
              setDailyStatusFilter(
                event.target.value as
                  | "all"
                  | "started"
                  | "success"
                  | "failed"
                  | "partial"
                  | "skipped"
              );
              setDailyStatusOverride(null);
              setStatsPage(1);
            }}
            disabled={actionBusy}
          >
            <option value="all">运行状态：全部</option>
            <option value="started">进行中</option>
            <option value="success">成功</option>
            <option value="failed">失败</option>
            <option value="partial">部分成功</option>
            <option value="skipped">已跳过</option>
          </select>
          <input
            value={dailyStatusCustomInput}
            onChange={(event) => setDailyStatusCustomInput(event.target.value)}
            placeholder="高级：自定义运行状态（可输入非法值做验收）"
            disabled={actionBusy}
            data-testid="settings-custom-daily-status"
          />
          <button
            type="button"
            onClick={onApplyCustomDailyStatus}
            disabled={actionBusy}
            data-testid="settings-apply-custom-daily-status"
          >
            应用状态
          </button>
          {dailyStatusOverride && (
            <span className="hint">当前自定义状态：{dailyStatusOverride}</span>
          )}
          <span className="hint">任务类型/状态筛选由后端聚合接口执行</span>
        </div>
        <RemoteState
          loading={statsLoading}
          error={statsError}
          empty={dailyStats.length === 0}
          loadingText="日统计加载中..."
          emptyText="当前筛选条件下暂无日统计。可切换任务类型或扩大时间范围后重试。"
          errorNextStep="下一步：点击“重试加载统计”；若仍失败，请先刷新日志确认同步记录是否存在。"
          emptyNextStep="下一步：切换任务类型为“全部”并放宽时间范围，再刷新统计。"
          onRetry={() => void loadSyncOverview()}
          retryLabel="重试加载统计"
        >
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>日期</th>
                  <th>任务类型</th>
                  <th>运行次数</th>
                  <th>成功率</th>
                  <th>平均成功条数</th>
                  <th>平均失败条数</th>
                </tr>
              </thead>
              <tbody>
                {dailyStats.map((item) => (
                  <tr key={`${item.day}-${item.job_type}`}>
                    <td>{item.day}</td>
                    <td>
                      <div className="title-cell">
                        <span>{formatJobType(item.job_type)}</span>
                        <small>{item.job_type}</small>
                      </div>
                    </td>
                    <td>{item.run_count}</td>
                    <td>
                      <div className="daily-rate">
                        <strong>{`${Math.round(item.success_rate * 100)}%`}</strong>
                        <div className="metric-meter">
                          <div
                            className={`metric-meter-fill ${item.success_rate < 0.8 ? "warn" : ""}`}
                            style={{ width: `${Math.round(item.success_rate * 100)}%` }}
                          />
                        </div>
                      </div>
                    </td>
                    <td>{item.avg_success_count.toFixed(2)}</td>
                    <td>{item.avg_fail_count.toFixed(2)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </RemoteState>
        <div className="inline-actions">
          <button
            type="button"
            onClick={() => setStatsPage((prev) => Math.max(prev - 1, 1))}
            disabled={actionBusy || statsLoading || statsPage <= 1}
          >
            上一页
          </button>
          <span className="hint">
            统计页 {statsPage}/{statsTotalPages} · 共 {statsTotal} 条
          </span>
          <button
            type="button"
            onClick={() => setStatsPage((prev) => Math.min(prev + 1, statsTotalPages))}
            disabled={actionBusy || statsLoading || statsPage >= statsTotalPages}
          >
            下一页
          </button>
          <button
            type="button"
            onClick={() => void loadSyncOverview()}
            disabled={actionBusy || statsLoading}
            data-testid="settings-refresh-stats"
          >
            刷新统计
          </button>
        </div>
      </article>
    </section>
  );
}
