import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { NoticeItem } from "../App";
import { PublishLogTable, type PublishSyncModeFilter } from "../components/publish/PublishLogTable";
import { RemoteState } from "../components/RemoteState";
import {
  getAiUsageMonthlySummary,
  getCollectionInsight,
  getDashboardMetrics,
  getPublishHistory,
  getSyncConfigSnapshot,
  retryDeadLetters,
  toUserMessage
} from "../services/ipc";
import type {
  AiUsageMonthlySummary,
  CollectionInsight,
  DashboardMetrics,
  PublishHistoryItem,
  RetryResult,
  SyncConfigSnapshot
} from "../types/contracts";
import { formatPercent, nowLocalIso } from "../utils/time";

interface Props {
  onNotify: (item: Omit<NoticeItem, "id">) => void;
}

interface ActionFeedback {
  level: "success" | "warning" | "error";
  message: string;
  nextStep?: string;
}

interface WeeklyUsageRow {
  week_key: string;
  week_label: string;
  calls: number;
  tokens_in: number;
  tokens_out: number;
  cost_cny: number;
}

function parseDeadLetterInput(input: string): string[] {
  const values = input
    .split(/[\s,，]+/)
    .map((item) => item.trim())
    .filter(Boolean);
  return [...new Set(values)];
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

function formatCny(value: number): string {
  return `¥${value.toFixed(2)}`;
}

function getIsoWeekKey(date: Date): string {
  const utc = new Date(Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()));
  const day = utc.getUTCDay() || 7;
  utc.setUTCDate(utc.getUTCDate() + 4 - day);
  const yearStart = new Date(Date.UTC(utc.getUTCFullYear(), 0, 1));
  const weekNo = Math.ceil((((utc.getTime() - yearStart.getTime()) / 86400000) + 1) / 7);
  return `${utc.getUTCFullYear()}-W${String(weekNo).padStart(2, "0")}`;
}

function monthDays(monthKey: string): number {
  const [yearRaw, monthRaw] = monthKey.split("-");
  const year = Number(yearRaw);
  const month = Number(monthRaw);
  if (!Number.isFinite(year) || !Number.isFinite(month) || month < 1 || month > 12) {
    return 30;
  }
  return new Date(year, month, 0).getDate();
}

function buildMonthWeekKeys(monthKey: string): string[] {
  const [yearRaw, monthRaw] = monthKey.split("-");
  const year = Number(yearRaw);
  const month = Number(monthRaw);
  if (!Number.isFinite(year) || !Number.isFinite(month) || month < 1 || month > 12) {
    return [];
  }
  const keys: string[] = [];
  const totalDays = new Date(year, month, 0).getDate();
  for (let day = 1; day <= totalDays; day += 1) {
    const key = getIsoWeekKey(new Date(year, month - 1, day));
    if (!keys.includes(key)) {
      keys.push(key);
    }
  }
  return keys;
}

export function SystemHealthPage({ onNotify }: Props) {
  const [metrics, setMetrics] = useState<DashboardMetrics | null>(null);
  const [insight, setInsight] = useState<CollectionInsight | null>(null);
  const [syncConfig, setSyncConfig] = useState<SyncConfigSnapshot | null>(null);
  const [aiUsage, setAiUsage] = useState<AiUsageMonthlySummary | null>(null);
  const [logs, setLogs] = useState<PublishHistoryItem[]>([]);
  const [showRetryableOnly, setShowRetryableOnly] = useState(false);
  const [syncModeFilter, setSyncModeFilter] = useState<PublishSyncModeFilter>("all");

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<ActionFeedback | null>(null);

  const [deadLetterIds, setDeadLetterIds] = useState("");
  const [retrying, setRetrying] = useState(false);

  const requestSeqRef = useRef(0);

  const loadHealthData = useCallback(async () => {
    const requestSeq = requestSeqRef.current + 1;
    requestSeqRef.current = requestSeq;
    setLoading(true);

    const to = nowLocalIso();
    const fromDate = new Date();
    fromDate.setDate(fromDate.getDate() - 7);
    const range = {
      from: fromDate.toISOString(),
      to
    };
    const historyFilters = {
      sync_mode: syncModeFilter === "all" ? undefined : syncModeFilter,
      retryable: showRetryableOnly ? true : undefined
    };

    const [metricsRes, insightRes, cfgRes, usageRes, logsRes] = await Promise.allSettled([
      getDashboardMetrics(),
      getCollectionInsight(),
      getSyncConfigSnapshot(),
      getAiUsageMonthlySummary(),
      getPublishHistory(range, { page: 1, page_size: 20 }, historyFilters)
    ]);

    if (requestSeq !== requestSeqRef.current) {
      return;
    }

    if (metricsRes.status === "fulfilled") {
      setMetrics(metricsRes.value);
    }
    if (insightRes.status === "fulfilled") {
      setInsight(insightRes.value);
    }
    if (cfgRes.status === "fulfilled") {
      setSyncConfig(cfgRes.value);
    }
    if (usageRes.status === "fulfilled") {
      setAiUsage(usageRes.value);
    }
    if (logsRes.status === "fulfilled") {
      setLogs(logsRes.value.items);
    } else {
      setLogs([]);
    }

    const hardErrors = [metricsRes, insightRes, cfgRes, usageRes, logsRes].filter(
      (result) => result.status === "rejected"
    );

    if (hardErrors.length > 0) {
      const firstError = hardErrors[0] as PromiseRejectedResult;
      setError(`系统健康数据加载失败：${toUserMessage(firstError.reason, "请求失败")}`);
    } else {
      setError(null);
    }

    setLoading(false);
  }, [showRetryableOnly, syncModeFilter]);

  useEffect(() => {
    void loadHealthData();
    return () => {
      requestSeqRef.current += 1;
    };
  }, [loadHealthData]);

  const onRetryDeadLetters = async () => {
    const ids = parseDeadLetterInput(deadLetterIds);
    if (ids.length === 0) {
      setFeedback({
        level: "warning",
        message: "未输入可回放 ID。",
        nextStep: "下一步：从失败日志复制 ID 后再执行回放。"
      });
      return;
    }

    setRetrying(true);
    setFeedback(null);
    try {
      const result: RetryResult = await retryDeadLetters(ids);
      const level = result.requeued === result.requested ? "success" : "warning";
      const message = `回放完成：重排 ${result.requeued}/${result.requested}，忽略 ${result.ignored}`;
      setFeedback({
        level,
        message,
        nextStep:
          result.requeued === result.requested
            ? "下一步：刷新系统健康数据确认状态变化。"
            : "下一步：检查 ID 是否来自失败日志后再次回放。"
      });
      onNotify({ level, message });
      setDeadLetterIds("");
    } catch (e) {
      const message = toUserMessage(e, "回放失败");
      setFeedback({
        level: "error",
        message,
        nextStep: "下一步：检查配置与ID有效性后重试。"
      });
      onNotify({ level: "error", message });
    } finally {
      setRetrying(false);
      await loadHealthData();
    }
  };

  const failedCount = useMemo(
    () => logs.filter((item) => item.state === "failed" || item.state === "partial").length,
    [logs]
  );

  const weeklyUsage = useMemo<WeeklyUsageRow[]>(() => {
    if (!aiUsage) {
      return [];
    }
    const weekOrder = buildMonthWeekKeys(aiUsage.month);
    if (weekOrder.length === 0) {
      return [];
    }
    const weekIndex = new Map<string, number>(weekOrder.map((key, idx) => [key, idx + 1]));
    const aggregate = new Map<string, WeeklyUsageRow>();
    for (const item of aiUsage.daily) {
      const date = new Date(`${item.day}T00:00:00`);
      if (Number.isNaN(date.getTime())) {
        continue;
      }
      const weekKey = getIsoWeekKey(date);
      if (!weekIndex.has(weekKey)) {
        continue;
      }
      const current =
        aggregate.get(weekKey) ??
        ({
          week_key: weekKey,
          week_label: `第${weekIndex.get(weekKey)}周`,
          calls: 0,
          tokens_in: 0,
          tokens_out: 0,
          cost_cny: 0
        } as WeeklyUsageRow);
      current.calls += item.calls;
      current.tokens_in += item.tokens_in;
      current.tokens_out += item.tokens_out;
      current.cost_cny += item.cost_cny;
      aggregate.set(weekKey, current);
    }
    return weekOrder
      .map((weekKey) => {
        const existing = aggregate.get(weekKey);
        if (existing) {
          return existing;
        }
        return {
          week_key: weekKey,
          week_label: `第${weekIndex.get(weekKey)}周`,
          calls: 0,
          tokens_in: 0,
          tokens_out: 0,
          cost_cny: 0
        };
      })
      .reverse();
  }, [aiUsage]);

  const projectedMonthCost = useMemo(() => {
    if (!aiUsage) {
      return 0;
    }
    const totalWeeks = buildMonthWeekKeys(aiUsage.month).length;
    const observedWeeks = weeklyUsage.filter((item) => item.cost_cny > 0 || item.calls > 0).length;
    if (totalWeeks <= 0 || observedWeeks <= 0) {
      return aiUsage.used_cny;
    }
    return (aiUsage.used_cny / observedWeeks) * totalWeeks;
  }, [aiUsage, weeklyUsage]);

  const projectedUsageRatio = useMemo(() => {
    if (!aiUsage || aiUsage.budget_limit_cny <= 0) {
      return 0;
    }
    return projectedMonthCost / aiUsage.budget_limit_cny;
  }, [aiUsage, projectedMonthCost]);

  const budgetAlertLevel = useMemo<"none" | "warning" | "error">(() => {
    if (!aiUsage) {
      return "none";
    }
    const currentRatio = aiUsage.usage_ratio;
    if (currentRatio >= 1 || projectedUsageRatio >= 1) {
      return "error";
    }
    if (currentRatio >= 0.8 || projectedUsageRatio >= 0.8) {
      return "warning";
    }
    return "none";
  }, [aiUsage, projectedUsageRatio]);

  const pendingPublish =
    (insight?.sync_pending ?? 0) + (insight?.sync_retry ?? 0) + (insight?.sync_not_started ?? 0);

  return (
    <section>
      <header className="page-header">
        <h1>系统健康</h1>
        <p>只展示系统状态、发布可靠性与失败恢复入口，不承载业务录入动作。</p>
      </header>

      <RemoteState
        loading={loading}
        error={error}
        empty={!metrics || !insight}
        loadingText="系统健康数据加载中..."
        emptyText="暂无系统健康数据"
        errorNextStep="下一步：点击“重试加载”；若持续失败，请检查本地数据库和同步配置。"
        emptyNextStep="下一步：先执行一次抓取与发布，再回到本页查看健康状态。"
        onRetry={() => void loadHealthData()}
        retryLabel="重试加载"
      >
        {metrics && insight && (
          <>
            <section className="card-grid">
              <article className="metric-card">
                <span>今日入箱</span>
                <strong>{metrics.today_collected}</strong>
              </article>
              <article className="metric-card">
                <span>待审核</span>
                <strong>{metrics.pending_review}</strong>
              </article>
              <article className="metric-card">
                <span>待发布</span>
                <strong>{pendingPublish}</strong>
              </article>
              <article className="metric-card">
                <span>预算占用</span>
                <strong>
                  {formatPercent(
                    aiUsage?.usage_ratio ?? metrics.budget_usage_ratio
                  )}
                </strong>
                <small>80%自动降载，100%自动熔断</small>
              </article>
            </section>

            <div className="settings-grid">
              <article className="panel-card">
                <h3>同步配置快照</h3>
                <div className="kv-row">
                  <span>模式</span>
                  <code>{syncConfig?.mode ?? "unknown"}</code>
                </div>
                <div className="kv-row">
                  <span>Notion 凭证</span>
                  <code>{statusLabel(syncConfig?.token_configured ?? null)}</code>
                </div>
                <div className="kv-row">
                  <span>Root Page ID</span>
                  <code>{statusLabel(syncConfig?.root_page_id_configured ?? null)}</code>
                </div>
                <div className="kv-row">
                  <span>Timezone</span>
                  <code>{syncConfig?.timezone ?? "-"}</code>
                </div>
                <div className="kv-row">
                  <span>分类增长上限</span>
                  <code>{syncConfig?.category_growth_limit ?? "-"}</code>
                </div>
              </article>

              <article className="panel-card">
                <h3>周监控与月底预测</h3>
                <p className="hint">按周统计花费，并估算本月月底总花费，支持预算预警。</p>
                <div className="kv-row">
                  <span>统计月份</span>
                  <code>{aiUsage?.month ?? "-"}</code>
                </div>
                <div className="kv-row">
                  <span>已用 / 上限</span>
                  <code>
                    {aiUsage ? `${formatCny(aiUsage.used_cny)} / ${formatCny(aiUsage.budget_limit_cny)}` : "-"}
                  </code>
                </div>
                <div className="kv-row">
                  <span>预算策略</span>
                  <code>&gt;=80% 自动降载 | &gt;=100% 自动熔断</code>
                </div>
                <div className="kv-row">
                  <span>周预测月底总花费</span>
                  <code>{aiUsage ? formatCny(projectedMonthCost) : "-"}</code>
                </div>
                <div className="kv-row">
                  <span>预测预算占用</span>
                  <code>{aiUsage ? formatPercent(projectedUsageRatio) : "-"}</code>
                </div>
                <div className="kv-row">
                  <span>剩余预算（当前）</span>
                  <code>{aiUsage ? formatCny(aiUsage.remaining_cny) : "-"}</code>
                </div>
                <div className="kv-row">
                  <span>本月总调用 / Token</span>
                  <code>
                    {aiUsage
                      ? `${aiUsage.calls} 次 | ${aiUsage.tokens_in.toLocaleString()} / ${aiUsage.tokens_out.toLocaleString()}`
                      : "-"}
                  </code>
                </div>
                <div className="kv-row">
                  <span>单次平均成本</span>
                  <code>{aiUsage ? formatCny(aiUsage.avg_cost_per_call_cny) : "-"}</code>
                </div>
                {budgetAlertLevel !== "none" && (
                  <div className={`state-panel ${budgetAlertLevel === "error" ? "error" : "warning"}`}>
                    <div>
                      {budgetAlertLevel === "error"
                        ? "预算熔断：当前或预测花费已达到/超过月预算上限，AI 生成将自动停用。"
                        : "预算降载：当前或预测花费已达到 80% 阈值，系统将自动降低 AI 调用量。"}
                    </div>
                    <small>
                      下一步：优先处理高价值条目；若已熔断，请下月自动恢复或手动提高预算上限。
                    </small>
                  </div>
                )}
                <div className="table-wrap compact">
                  <table aria-label="本月周花费明细">
                    <thead>
                      <tr>
                        <th scope="col">周</th>
                        <th scope="col">调用</th>
                        <th scope="col">Token(入/出)</th>
                        <th scope="col">成本</th>
                      </tr>
                    </thead>
                    <tbody>
                      {weeklyUsage.length === 0 ? (
                        <tr>
                          <td colSpan={4}>暂无调用记录</td>
                        </tr>
                      ) : (
                        weeklyUsage.map((item) => (
                          <tr key={item.week_key}>
                            <td>{item.week_label}</td>
                            <td>{item.calls}</td>
                            <td>
                              {item.tokens_in.toLocaleString()} / {item.tokens_out.toLocaleString()}
                            </td>
                            <td>{formatCny(item.cost_cny)}</td>
                          </tr>
                        ))
                      )}
                    </tbody>
                  </table>
                </div>
              </article>

              <article className="panel-card">
                <h3>失败回放</h3>
                <p className="hint">失败或部分失败日志可复制 ID，在此执行 dead letter 回放。</p>
                <input
                  className="text-input"
                  placeholder="输入失败ID，支持逗号或空格分隔"
                  value={deadLetterIds}
                  onChange={(e) => setDeadLetterIds(e.target.value)}
                  disabled={retrying}
                />
                <div className="inline-actions">
                  <button
                    type="button"
                    className="button-primary"
                    onClick={onRetryDeadLetters}
                    disabled={retrying}
                    data-testid="health-retry-dead-letters"
                  >
                    {retrying ? "回放中..." : "执行回放"}
                  </button>
                  <span className="hint">当前失败日志数：{failedCount}</span>
                </div>
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
              </article>
            </div>

            <article className="panel-card">
              <h3>最近发布日志</h3>
              <PublishLogTable
                logs={logs}
                showRetryableOnly={showRetryableOnly}
                onShowRetryableOnlyChange={setShowRetryableOnly}
                syncModeFilter={syncModeFilter}
                onSyncModeFilterChange={setSyncModeFilter}
                emptyText="系统健康页当前筛选下暂无发布日志。"
                emptyNextStep="下一步：取消筛选或等待下一次发布任务后重试。"
                tableAriaLabel="系统健康最近发布日志"
              />
            </article>
          </>
        )}
      </RemoteState>
    </section>
  );
}
