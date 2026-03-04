import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { NoticeItem } from "../App";
import { PublishLogTable, type PublishSyncModeFilter } from "../components/publish/PublishLogTable";
import { RemoteState } from "../components/RemoteState";
import {
  getPublishHistory,
  getPublishQueue,
  publishApprovedToNotion,
  toUserMessage
} from "../services/ipc";
import type { PublishHistoryItem, PublishResult, PublishTask } from "../types/contracts";
import { nowLocalIso } from "../utils/time";

interface Props {
  onNotify: (item: Omit<NoticeItem, "id">) => void;
  onOpenHealth: () => void;
}

interface ActionFeedback {
  level: "success" | "warning" | "error";
  message: string;
  nextStep?: string;
}

function createRange(days: number) {
  const to = nowLocalIso();
  const fromDate = new Date();
  fromDate.setDate(fromDate.getDate() - days);
  return {
    from: fromDate.toISOString(),
    to
  };
}

export function PublishCenterPage({ onNotify, onOpenHealth }: Props) {
  const [syncLimitInput, setSyncLimitInput] = useState("30");
  const [syncing, setSyncing] = useState(false);
  const [lastSummary, setLastSummary] = useState<PublishResult | null>(null);
  const [feedback, setFeedback] = useState<ActionFeedback | null>(null);

  const [logs, setLogs] = useState<PublishHistoryItem[]>([]);
  const [queueItems, setQueueItems] = useState<PublishTask[]>([]);
  const [logsLoading, setLogsLoading] = useState(true);
  const [logsError, setLogsError] = useState<string | null>(null);
  const [showRetryableOnly, setShowRetryableOnly] = useState(false);
  const [syncModeFilter, setSyncModeFilter] = useState<PublishSyncModeFilter>("all");
  const requestSeqRef = useRef(0);

  const loadPublishData = useCallback(async () => {
    const requestSeq = requestSeqRef.current + 1;
    requestSeqRef.current = requestSeq;
    setLogsLoading(true);

    const range = createRange(7);
    const historyFilters = {
      sync_mode: syncModeFilter === "all" ? undefined : syncModeFilter,
      retryable: showRetryableOnly ? true : undefined
    };
    const [logsResult, statsResult] = await Promise.allSettled([
      getPublishHistory(range, { page: 1, page_size: 20 }, historyFilters),
      getPublishQueue({ page: 1, page_size: 50 })
    ]);

    if (requestSeq !== requestSeqRef.current) {
      return;
    }

    if (logsResult.status === "fulfilled") {
      setLogs(logsResult.value.items);
      setLogsError(null);
    } else {
      setLogsError(`发布日志加载失败：${toUserMessage(logsResult.reason, "获取日志失败")}`);
    }

    if (statsResult.status === "fulfilled") {
      setQueueItems(statsResult.value.items);
    } else {
      setQueueItems([]);
    }

    setLogsLoading(false);
  }, [showRetryableOnly, syncModeFilter]);

  useEffect(() => {
    void loadPublishData();
    return () => {
      requestSeqRef.current += 1;
    };
  }, [loadPublishData]);

  const onPublishNow = async () => {
    const normalized = syncLimitInput.trim();
    const parsedLimit = normalized.length === 0 ? undefined : Number(normalized);
    if (parsedLimit !== undefined && (!Number.isFinite(parsedLimit) || parsedLimit <= 0)) {
      setFeedback({
        level: "warning",
        message: "发布上限无效，请输入大于 0 的数字。",
        nextStep: "下一步：修正上限后重新点击“立即发布到 Notion”。"
      });
      return;
    }

    setSyncing(true);
    setFeedback(null);
    try {
      const summary = await publishApprovedToNotion(
        parsedLimit === undefined ? undefined : Math.floor(parsedLimit)
      );
      setLastSummary(summary);
      const level = summary.failed > 0 ? "warning" : "success";
      const message = `发布完成：成功 ${summary.succeeded} / 失败 ${summary.failed}（尝试 ${summary.attempted}）`;
      setFeedback({
        level,
        message,
        nextStep:
          summary.failed > 0
            ? "下一步：打开“系统健康”查看失败原因并执行回放。"
            : "下一步：去 Notion 查看最新分类与周分区条目。"
      });
      onNotify({ level, message });
    } catch (e) {
      const message = toUserMessage(e, "Notion 发布失败");
      const nextStep = message.includes("NOTION_ROOT_PAGE_ID")
        ? "下一步：去“系统健康”确认 Root Page ID 已配置，再重试发布。"
        : message.includes("NOTION_TOKEN")
          ? "下一步：去“系统健康”确认 Notion 凭证已配置，再重试发布。"
          : "下一步：先检查同步配置和网络，再重试发布。";
      setFeedback({
        level: "error",
        message,
        nextStep
      });
      onNotify({ level: "error", message });
    } finally {
      setSyncing(false);
      await loadPublishData();
    }
  };

  const weekSuccessRate = useMemo(() => {
    if (logs.length === 0) {
      return null;
    }
    const successCount = logs.reduce((acc, item) => acc + item.success_count, 0);
    const failCount = logs.reduce((acc, item) => acc + item.fail_count, 0);
    const total = successCount + failCount;
    if (total <= 0) {
      return 0;
    }
    return Math.round((successCount / total) * 100);
  }, [logs]);

  return (
    <section>
      <header className="page-header">
        <h1>发布中心</h1>
        <p>只负责把已通过内容即时发布到 Notion，保证目录结构与条目模板稳定。</p>
      </header>

      <article className="panel-card">
        <h3>立即发布</h3>
        <p className="hint">结构：Personal / 分类 / ISO 周 / 条目页（结论摘要 + 关键要点 + 原文链接 + 标签）。</p>
        <div className="inline-actions">
          <input
            className="text-input"
            type="number"
            min={1}
            value={syncLimitInput}
            onChange={(e) => setSyncLimitInput(e.target.value)}
            disabled={syncing}
            aria-label="发布上限"
          />
          <button
            type="button"
            className="button-primary"
            onClick={onPublishNow}
            disabled={syncing}
            data-testid="publish-run-now"
          >
            {syncing ? "发布中..." : "立即发布到 Notion"}
          </button>
          <button
            type="button"
            className="button-secondary"
            onClick={onOpenHealth}
            data-testid="publish-open-health"
          >
            打开系统健康
          </button>
        </div>
      </article>

      {feedback && (
        <div
          className={`state-panel ${feedback.level === "error" ? "error" : feedback.level === "warning" ? "warning" : ""}`}
          role={feedback.level === "error" ? "alert" : "status"}
          aria-live={feedback.level === "error" ? "assertive" : "polite"}
        >
          <div>{feedback.message}</div>
          {feedback.nextStep && <small>{feedback.nextStep}</small>}
        </div>
      )}

      <section className="card-grid">
        <article className="metric-card">
          <span>最近一次发布</span>
          <strong>{lastSummary ? `${lastSummary.succeeded}/${lastSummary.attempted}` : "-"}</strong>
          <small>{lastSummary ? `失败 ${lastSummary.failed} 条` : "尚未触发"}</small>
        </article>
        <article className="metric-card">
          <span>7天发布成功率</span>
          <strong>{weekSuccessRate === null ? "-" : `${weekSuccessRate}%`}</strong>
          <small>仅统计 notion_sync_once</small>
        </article>
        <article className="metric-card">
          <span>待回放失败</span>
          <strong>
            {queueItems.filter((item) => item.state === "failed" || item.state === "retry").length}
          </strong>
          <small>失败可在系统健康页处理</small>
        </article>
        <article className="metric-card">
          <span>发布策略</span>
          <strong>即时发布</strong>
          <small>审核通过后尽快可见</small>
        </article>
      </section>

      <RemoteState
        loading={logsLoading}
        error={logsError}
        empty={logs.length === 0 && !showRetryableOnly && syncModeFilter === "all"}
        loadingText="发布日志加载中..."
        emptyText="暂无发布日志"
        errorNextStep="下一步：点击“重试加载”；若持续失败，请检查本地数据库配置。"
        emptyNextStep="下一步：先执行一次“立即发布到 Notion”。"
        onRetry={() => void loadPublishData()}
        retryLabel="重试加载"
      >
        <article className="panel-card">
          <h3>最近发布日志</h3>
          <PublishLogTable
            logs={logs}
            showRetryableOnly={showRetryableOnly}
            onShowRetryableOnlyChange={setShowRetryableOnly}
            syncModeFilter={syncModeFilter}
            onSyncModeFilterChange={setSyncModeFilter}
            emptyText="最近 7 天暂无发布日志。"
            emptyNextStep="下一步：先执行一次“立即发布到 Notion”，再按筛选查看结果。"
            tableAriaLabel="发布中心最近发布日志"
          />
        </article>
      </RemoteState>
    </section>
  );
}
