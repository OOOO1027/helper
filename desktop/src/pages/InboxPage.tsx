import { useCallback, useEffect, useRef, useState } from "react";
import type { NoticeItem } from "../App";
import { RemoteState } from "../components/RemoteState";
import { getCollectionInsight, ingestXhsIncremental, toUserMessage } from "../services/ipc";
import type { CollectionInsight } from "../types/contracts";

interface Props {
  onNotify: (item: Omit<NoticeItem, "id">) => void;
  onOpenReview: () => void;
  onOpenPublish: () => void;
}

interface ActionFeedback {
  level: "success" | "warning" | "error";
  message: string;
  nextStep?: string;
}

function formatReviewState(state: string): string {
  if (state === "pending" || state === "processing") {
    return "待审核";
  }
  if (state === "done") {
    return "已通过";
  }
  if (state === "rejected") {
    return "已驳回";
  }
  return state;
}

export function InboxPage({ onNotify, onOpenReview, onOpenPublish }: Props) {
  const [insight, setInsight] = useState<CollectionInsight | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [lastSyncedAt, setLastSyncedAt] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<ActionFeedback | null>(null);
  const mountedRef = useRef(true);

  const loadInsight = useCallback(async (silent = false) => {
    if (!silent) {
      setLoading(true);
    }
    try {
      const data = await getCollectionInsight();
      if (!mountedRef.current) {
        return;
      }
      setInsight(data);
      setError(null);
    } catch (e) {
      if (!mountedRef.current) {
        return;
      }
      const message = toUserMessage(e, "收件箱数据加载失败");
      setError(`收件箱数据加载失败：${message}`);
    } finally {
      if (mountedRef.current) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    void loadInsight();
    return () => {
      mountedRef.current = false;
    };
  }, [loadInsight]);

  const onIngest = async () => {
    setSyncing(true);
    setFeedback(null);
    try {
      const result = await ingestXhsIncremental();
      const message = `抓取完成：新增 ${result.stored}，扫描 ${result.fetched}`;
      setLastSyncedAt(new Date().toLocaleString("zh-CN"));
      setFeedback({
        level: "success",
        message,
        nextStep: "下一步：进入“审核”处理低置信内容，系统会即时发布通过项。",
      });
      onNotify({ level: "success", message });
      await loadInsight(true);
    } catch (e) {
      const message = toUserMessage(e, "小红书增量抓取失败");
      setFeedback({
        level: "error",
        message,
        nextStep: "下一步：检查小红书登录态与抓取权限后重试。",
      });
      onNotify({ level: "error", message });
    } finally {
      setSyncing(false);
    }
  };

  const pendingPublish =
    (insight?.sync_pending ?? 0) + (insight?.sync_retry ?? 0) + (insight?.sync_not_started ?? 0);

  return (
    <section>
      <header className="page-header">
        <h1>收件箱</h1>
        <p>只处理新增抓取：启动自动增量 + 手动补拉，默认来源为小红书点赞与收藏。</p>
      </header>

      <div className="inline-actions">
        <button
          type="button"
          className="button-primary"
          onClick={onIngest}
          disabled={syncing}
          data-testid="inbox-ingest-xhs"
        >
          {syncing ? "抓取中..." : "抓取小红书增量"}
        </button>
        <button
          type="button"
          className="button-secondary"
          onClick={onOpenReview}
          data-testid="inbox-open-review"
        >
          进入审核
        </button>
        <button
          type="button"
          className="button-secondary"
          onClick={onOpenPublish}
          data-testid="inbox-open-publish"
        >
          查看发布中心
        </button>
        <span className="hint">目标：高置信自动发布，低置信进入人工审核。</span>
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

      {lastSyncedAt && (
        <article className="panel-card">
          <h3>最近一次抓取</h3>
          <p className="hint">{lastSyncedAt}</p>
        </article>
      )}

      <RemoteState
        loading={loading}
        error={error}
        empty={!insight}
        loadingText="收件箱状态加载中..."
        emptyText="暂无收件箱数据"
        errorNextStep="下一步：点击“重试加载”；若仍失败，请先执行一次抓取。"
        emptyNextStep="下一步：点击“抓取小红书增量”，然后返回查看入箱结果。"
        onRetry={() => void loadInsight()}
        retryLabel="重试加载"
      >
        {insight && (
          <>
            <section className="card-grid">
              <article className="metric-card">
                <span>今日入箱</span>
                <strong>{insight.today_total}</strong>
                <small>来源：小红书增量抓取</small>
              </article>
              <article className="metric-card">
                <span>待审核</span>
                <strong>{insight.review_pending}</strong>
                <small>需要人工确认</small>
              </article>
              <article className="metric-card">
                <span>待发布</span>
                <strong>{pendingPublish}</strong>
                <small>待写入 Notion</small>
              </article>
              <article className="metric-card">
                <span>发布失败</span>
                <strong>{insight.sync_failed}</strong>
                <small>可在系统健康页回放</small>
              </article>
            </section>

            <article className="panel-card">
              <h3>来源分布</h3>
              <p className="hint">
                {insight.sources.map((item) => `${item.source}:${item.count}`).join(" / ") ||
                  "暂无数据"}
              </p>
            </article>

            <article className="panel-card">
              <h3>最近入箱条目</h3>
              {insight.recent_items.length === 0 ? (
                <p className="hint">暂无最近条目</p>
              ) : (
                <div className="table-wrap compact">
                  <table>
                    <thead>
                      <tr>
                        <th>标题</th>
                        <th>审核状态</th>
                        <th>发布状态</th>
                        <th>时间</th>
                      </tr>
                    </thead>
                    <tbody>
                      {insight.recent_items.map((item) => (
                        <tr key={`${item.source}-${item.collected_at}-${item.title}`}>
                          <td>{item.title}</td>
                          <td>{formatReviewState(item.review_state)}</td>
                          <td>{item.sync_state}</td>
                          <td>{item.collected_at}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </article>
          </>
        )}
      </RemoteState>
    </section>
  );
}
