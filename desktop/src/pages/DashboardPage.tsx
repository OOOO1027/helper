import { useCallback, useEffect, useRef, useState } from "react";
import type { NoticeItem } from "../App";
import { RemoteState } from "../components/RemoteState";
import { getCollectionInsight, getDashboardMetrics, toUserMessage } from "../services/ipc";
import type { CollectionInsight, DashboardMetrics } from "../types/contracts";
import { formatPercent } from "../utils/time";

interface Props {
  onNotify: (item: Omit<NoticeItem, "id">) => void;
  onOpenReview: () => void;
}

function formatReviewState(state: string): string {
  if (state === "pending" || state === "processing") {
    return "待审核";
  }
  if (state === "done") {
    return "已完成";
  }
  if (state === "rejected") {
    return "异常";
  }
  if (state === "not_in_queue") {
    return "直过（无需审核）";
  }
  return state;
}

function formatSyncState(state: string): string {
  if (state === "success") {
    return "已写入 Notion";
  }
  if (state === "pending") {
    return "待同步 Notion";
  }
  if (state === "retry") {
    return "重试中";
  }
  if (state === "failed") {
    return "同步失败";
  }
  if (state === "not_started") {
    return "尚未发起同步";
  }
  return state;
}

export function DashboardPage({ onNotify, onOpenReview }: Props) {
  const [metrics, setMetrics] = useState<DashboardMetrics | null>(null);
  const [insight, setInsight] = useState<CollectionInsight | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const mountedRef = useRef(true);

  const loadMetrics = useCallback(
    async (silent = false) => {
      if (!silent) {
        setLoading(true);
      }
      try {
        const [metricsRes, insightRes] = await Promise.allSettled([
          getDashboardMetrics(),
          getCollectionInsight()
        ]);
        if (!mountedRef.current) {
          return;
        }
        if (metricsRes.status === "rejected") {
          throw metricsRes.reason;
        }
        setMetrics(metricsRes.value);
        if (insightRes.status === "fulfilled") {
          setInsight(insightRes.value);
        } else {
          setInsight(null);
        }
        setError(null);
      } catch (e) {
        if (!mountedRef.current) {
          return;
        }
        const message = toUserMessage(e, "仪表盘加载失败");
        setError(`仪表盘加载失败：${message}`);
        onNotify({ level: "error", message: `仪表盘加载失败：${message}` });
      } finally {
        if (mountedRef.current) {
          setLoading(false);
        }
      }
    },
    [onNotify]
  );

  useEffect(() => {
    mountedRef.current = true;
    loadMetrics().catch(() => {
      if (mountedRef.current) {
        setLoading(false);
      }
    });

    return () => {
      mountedRef.current = false;
    };
  }, [loadMetrics]);

  return (
    <section>
      <header className="page-header">
        <h1>仪表盘</h1>
        <p>口径说明：今日采集是当日新增；待审核是跨天累计队列，不是同一分母。</p>
      </header>
      <RemoteState
        loading={loading}
        error={error}
        empty={!metrics}
        loadingText="仪表盘加载中..."
        emptyText="暂未获取到仪表盘数据"
        errorNextStep="下一步：点击“重试加载”；若持续失败，请先完成一次导入后再刷新。"
        emptyNextStep="下一步：先到“导入中心”完成导入，再返回查看状态。"
        onRetry={() => void loadMetrics()}
        retryLabel="重试加载"
      >
        {metrics && (
          <>
            <div className="card-grid">
              <article className="metric-card">
                <span>今日新增采集</span>
                <strong>{metrics.today_collected}</strong>
              </article>
              <article className="metric-card">
                <span>待审核队列总量</span>
                <strong>{metrics.pending_review}</strong>
              </article>
              <article className="metric-card">
                <span>分类准确率</span>
                <strong>{formatPercent(metrics.classification_accuracy)}</strong>
                <div className="metric-meter">
                  <div
                    className="metric-meter-fill"
                    style={{ width: `${Math.round(metrics.classification_accuracy * 100)}%` }}
                  />
                </div>
              </article>
              <article className="metric-card">
                <span>摘要可用性</span>
                <strong>{formatPercent(metrics.summary_usability)}</strong>
                <div className="metric-meter">
                  <div
                    className="metric-meter-fill"
                    style={{ width: `${Math.round(metrics.summary_usability * 100)}%` }}
                  />
                </div>
              </article>
              <article className="metric-card wide">
                <span>预算使用率</span>
                <strong>{formatPercent(metrics.budget_usage_ratio)}</strong>
                <div className="metric-meter">
                  <div
                    className="metric-meter-fill warn"
                    style={{ width: `${Math.round(metrics.budget_usage_ratio * 100)}%` }}
                  />
                </div>
                <small>预算守卫触发后会自动降级低优先任务</small>
                <div className="metric-actions">
                  <button type="button" onClick={onOpenReview}>
                    继续审核
                  </button>
                </div>
              </article>
            </div>
            {insight && (
              <section className="flow-card">
                <header>
                  <h3>今日采集去向总览（{insight.day}）</h3>
                  <p>
                    {insight.today_total} = 待审 {insight.review_pending} + 已完成 {insight.review_done} + 异常{" "}
                    {insight.review_rejected} + 直过 {insight.direct_no_review}
                  </p>
                </header>
                <div className="summary-grid">
                  <div>
                    <span>来源分布</span>
                    <strong>{insight.sources.map((item) => `${item.source}:${item.count}`).join(" / ") || "-"}</strong>
                  </div>
                  <div>
                    <span>Notion 已成功</span>
                    <strong>{insight.sync_success}</strong>
                  </div>
                  <div>
                    <span>Notion 待同步</span>
                    <strong>{insight.sync_pending + insight.sync_retry + insight.sync_not_started}</strong>
                  </div>
                  <div>
                    <span>Notion 失败</span>
                    <strong>{insight.sync_failed}</strong>
                  </div>
                </div>
                {insight.recent_items.length > 0 && (
                  <div className="table-wrap compact">
                    <table>
                      <thead>
                        <tr>
                          <th>来源</th>
                          <th>标题</th>
                          <th>审核状态</th>
                          <th>Notion状态</th>
                          <th>采集时间</th>
                        </tr>
                      </thead>
                      <tbody>
                        {insight.recent_items.map((item) => (
                          <tr key={`${item.source}-${item.collected_at}-${item.title}`}>
                            <td>{item.source}</td>
                            <td>{item.title}</td>
                            <td>{formatReviewState(item.review_state)}</td>
                            <td>{formatSyncState(item.sync_state)}</td>
                            <td>{item.collected_at}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </section>
            )}
          </>
        )}
      </RemoteState>
    </section>
  );
}
