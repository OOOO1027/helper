import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { NoticeItem } from "../App";
import { RemoteState } from "../components/RemoteState";
import { ReviewDrawer } from "../components/ReviewDrawer";
import { ReviewExceptionFilters } from "../components/review/ReviewExceptionFilters";
import { ReviewSessionPanel } from "../components/review/ReviewSessionPanel";
import { ReviewTabs } from "../components/review/ReviewTabs";
import {
  getReviewQueue,
  publishApprovedToNotion,
  retryFailedItems,
  toUserMessage,
  updateReviewItem
} from "../services/ipc";
import type { ReviewItem, ReviewTab } from "../types/contracts";
import type {
  ActionFeedback,
  BatchFailure,
  Decision,
  PriorityFilter,
  SessionActionLog,
  SortMode,
  TabCounts
} from "../components/review/types";

interface Props {
  onNotify: (item: Omit<NoticeItem, "id">) => void;
  forceTab?: ReviewTab;
  forceToken?: number;
}

const REVIEW_QUERY_KEY = "helper.review.query";
const REVIEW_SORT_KEY = "helper.review.sort";
const REVIEW_REASON_KEY = "helper.review.reason";
const REVIEW_PRIORITY_KEY = "helper.review.priority";
const AUTO_SYNC_LIMIT = 20;

function tabToFilter(tab: ReviewTab): string {
  if (tab === "pending") {
    return "pending";
  }
  if (tab === "exception") {
    return "rejected";
  }
  return "done";
}

function priorityTone(priority: number): "low" | "mid" | "high" {
  if (priority >= 0.75) {
    return "high";
  }
  if (priority >= 0.45) {
    return "mid";
  }
  return "low";
}

function formatReviewState(state: string): string {
  if (state === "pending" || state === "review" || state === "processing") {
    return "待审核";
  }
  if (state === "done") {
    return "已完成";
  }
  if (state === "failed" || state === "exception" || state === "rejected") {
    return "异常";
  }
  if (state === "partial") {
    return "部分成功";
  }
  if (state === "success") {
    return "成功";
  }
  return state;
}

function reviewStateTone(state: string): "success" | "warning" | "error" | "neutral" {
  if (state === "success" || state === "done") {
    return "success";
  }
  if (state === "failed" || state === "exception" || state === "rejected") {
    return "error";
  }
  if (state === "partial" || state === "pending" || state === "review" || state === "processing") {
    return "warning";
  }
  return "neutral";
}

export function ReviewQueuePage({ onNotify, forceTab, forceToken }: Props) {
  const [tab, setTab] = useState<ReviewTab>("pending");
  const [items, setItems] = useState<ReviewItem[]>([]);
  const [tabCounts, setTabCounts] = useState<TabCounts>({
    pending: 0,
    exception: 0,
    done: 0
  });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [checkedIds, setCheckedIds] = useState<string[]>([]);
  const [query, setQuery] = useState<string>(() => localStorage.getItem(REVIEW_QUERY_KEY) ?? "");
  const [sortMode, setSortMode] = useState<SortMode>(
    () => (localStorage.getItem(REVIEW_SORT_KEY) as SortMode | null) ?? "priority_desc"
  );
  const [reasonFilter, setReasonFilter] = useState<string>(
    () => localStorage.getItem(REVIEW_REASON_KEY) ?? "all"
  );
  const [priorityFilter, setPriorityFilter] = useState<PriorityFilter>(
    () => (localStorage.getItem(REVIEW_PRIORITY_KEY) as PriorityFilter | null) ?? "all"
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState(false);
  const [sessionStartedAt] = useState<number>(() => Date.now());
  const [sessionNow, setSessionNow] = useState<number>(() => Date.now());
  const [processedCount, setProcessedCount] = useState(0);
  const [sessionLogs, setSessionLogs] = useState<SessionActionLog[]>([]);
  const [lastFailures, setLastFailures] = useState<BatchFailure[]>([]);
  const [actionFeedback, setActionFeedback] = useState<ActionFeedback | null>(null);
  const requestSeqRef = useRef(0);

  const visibleItems = useMemo(() => {
    const reasonFiltered =
      tab === "exception" && reasonFilter !== "all"
        ? items.filter((item) => item.reason === reasonFilter)
        : items;
    const priorityFiltered =
      tab === "exception" && priorityFilter !== "all"
        ? reasonFiltered.filter((item) => priorityTone(item.priority) === priorityFilter)
        : reasonFiltered;
    const term = query.trim().toLowerCase();
    const textFiltered = term
      ? priorityFiltered.filter((item) =>
          [item.title, item.reason, item.id, item.normalizedId].join(" ").toLowerCase().includes(term)
        )
      : priorityFiltered;

    return [...textFiltered].sort((a, b) =>
      sortMode === "priority_desc" ? b.priority - a.priority : a.priority - b.priority
    );
  }, [items, priorityFilter, query, reasonFilter, sortMode, tab]);

  const reasonStats = useMemo(() => {
    const counter = new Map<string, number>();
    for (const item of items) {
      counter.set(item.reason, (counter.get(item.reason) ?? 0) + 1);
    }
    return [...counter.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8);
  }, [items]);

  const selectedItem = useMemo(
    () => visibleItems.find((item) => item.id === selectedId) ?? null,
    [visibleItems, selectedId]
  );

  const allVisibleChecked =
    visibleItems.length > 0 && visibleItems.every((item) => checkedIds.includes(item.id));
  const elapsedMinutes = Math.max((sessionNow - sessionStartedAt) / 60000, 0.1);
  const throughput = processedCount / elapsedMinutes;
  const projected15m = Math.round(throughput * 15);
  const targetProgress = Math.min((projected15m / 90) * 100, 100);
  const targetGap = Math.max(90 - projected15m, 0);
  const targetReached = projected15m >= 90;

  const reload = useCallback(
    async (preferredId?: string) => {
      const requestSeq = requestSeqRef.current + 1;
      requestSeqRef.current = requestSeq;
      setLoading(true);
      setError(null);

      try {
        const [queueResult, countResult] = await Promise.allSettled([
          getReviewQueue({
            filter: { state: tabToFilter(tab) },
            page: { page: 1, page_size: 20 }
          }),
          Promise.all([
            getReviewQueue({
              filter: { state: "pending" },
              page: { page: 1, page_size: 1 }
            }),
            getReviewQueue({
              filter: { state: "rejected" },
              page: { page: 1, page_size: 1 }
            }),
            getReviewQueue({
              filter: { state: "done" },
              page: { page: 1, page_size: 1 }
            })
          ])
        ]);
        if (requestSeq !== requestSeqRef.current) {
          return;
        }
        if (queueResult.status === "rejected") {
          throw queueResult.reason;
        }
        const res = queueResult.value;
        if (countResult.status === "fulfilled") {
          const [pendingRes, exceptionRes, doneRes] = countResult.value;
          setTabCounts({
            pending: pendingRes.total,
            exception: exceptionRes.total,
            done: doneRes.total
          });
        }
        setItems(res.items);
        const hasPreferred = preferredId && res.items.some((item) => item.id === preferredId);
        setSelectedId(hasPreferred ? preferredId ?? null : res.items[0]?.id ?? null);
        setCheckedIds([]);
      } catch (e) {
        if (requestSeq !== requestSeqRef.current) {
          return;
        }
        setError(`审核队列加载失败：${toUserMessage(e, "请求失败")}`);
      } finally {
        if (requestSeq === requestSeqRef.current) {
          setLoading(false);
        }
      }
    },
    [tab]
  );

  useEffect(() => {
    void reload();
    return () => {
      requestSeqRef.current += 1;
    };
  }, [reload]);

  useEffect(() => {
    if (forceTab) {
      setTab(forceTab);
    }
  }, [forceTab, forceToken]);

  useEffect(() => {
    localStorage.setItem(REVIEW_QUERY_KEY, query);
  }, [query]);

  useEffect(() => {
    localStorage.setItem(REVIEW_SORT_KEY, sortMode);
  }, [sortMode]);

  useEffect(() => {
    localStorage.setItem(REVIEW_REASON_KEY, reasonFilter);
  }, [reasonFilter]);

  useEffect(() => {
    localStorage.setItem(REVIEW_PRIORITY_KEY, priorityFilter);
  }, [priorityFilter]);

  useEffect(() => {
    if (tab !== "exception" && reasonFilter !== "all") {
      setReasonFilter("all");
    }
    if (tab !== "exception" && priorityFilter !== "all") {
      setPriorityFilter("all");
    }
  }, [priorityFilter, reasonFilter, tab]);

  useEffect(() => {
    if (selectedId && !visibleItems.some((item) => item.id === selectedId)) {
      setSelectedId(visibleItems[0]?.id ?? null);
    }
  }, [selectedId, visibleItems]);

  useEffect(() => {
    const timer = window.setInterval(() => setSessionNow(Date.now()), 10000);
    return () => window.clearInterval(timer);
  }, []);

  const appendSessionLog = (action: string, detail: string) => {
    setSessionLogs((prev) => [
      {
        id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
        at: Date.now(),
        action,
        detail
      },
      ...prev
    ].slice(0, 8));
  };

  const decisionLabel = (decision: Decision): string => {
    if (decision === "approved") {
      return "通过";
    }
    if (decision === "rejected") {
      return "驳回";
    }
    return "标记重复";
  };

  const applyDecision = async (ids: string[], decision: Decision, preferredId?: string | null) => {
    if (ids.length === 0) {
      return;
    }

    setActionBusy(true);
    setActionFeedback(null);
    let success = 0;
    let failed = 0;
    const failedItems: BatchFailure[] = [];
    try {
      for (const id of ids) {
        try {
          await updateReviewItem({
            id,
            patch: {
              state: decision === "approved" ? "done" : "rejected",
              note: `manual:${decision}`
            }
          });
          success += 1;
        } catch (e) {
          failed += 1;
          failedItems.push({
            id,
            message: toUserMessage(e, "更新失败")
          });
        }
      }

      if (success > 0) {
        setProcessedCount((prev) => prev + success);
      }
      if (failed === 0) {
        setActionFeedback({
          level: "success",
          message: `已${decisionLabel(decision)} ${success} 条`,
          nextStep: "下一步：继续处理下一批，或切换“异常”标签检查待重试项。"
        });
        onNotify({ level: "success", message: `已${decisionLabel(decision)} ${success} 条` });
      } else {
        setActionFeedback({
          level: "warning",
          message: `已${decisionLabel(decision)} ${success} 条，失败 ${failed} 条`,
          nextStep: "下一步：先查看下方失败明细，再点击“批量重试”处理失败项。"
        });
        onNotify({
          level: "warning",
          message: `已${decisionLabel(decision)} ${success} 条，失败 ${failed} 条`
        });
      }

      appendSessionLog(`批量${decisionLabel(decision)}`, `成功 ${success} 条，失败 ${failed} 条`);
      setLastFailures(failedItems);
      await reload(preferredId ?? undefined);
        setCheckedIds([]);
        if (success > 0) {
          try {
          const syncSummary = await publishApprovedToNotion(AUTO_SYNC_LIMIT);
          const syncText = `Notion 同步：成功 ${syncSummary.succeeded} / 失败 ${syncSummary.failed}（尝试 ${syncSummary.attempted}）`;
          appendSessionLog("自动同步 Notion", syncText);
          onNotify({
            level: syncSummary.failed > 0 ? "warning" : "success",
            message: syncText
          });
          setActionFeedback((prev) => {
            const merged = prev?.message ? `${prev.message}；${syncText}` : syncText;
            return {
              level: syncSummary.failed > 0 ? "warning" : prev?.level ?? "success",
              message: merged,
              nextStep:
                syncSummary.failed > 0
                  ? "下一步：去“系统健康”查看失败详情并回放。"
                  : "下一步：可到“系统健康”确认同步日志。"
            };
          });
        } catch (syncError) {
          const syncMessage = toUserMessage(syncError, "Notion 自动同步失败");
          appendSessionLog("自动同步 Notion", `失败：${syncMessage}`);
          onNotify({
            level: "warning",
            message: `Notion 自动同步失败：${syncMessage}`
          });
          setActionFeedback((prev) => {
            const merged = prev?.message
              ? `${prev.message}；Notion 自动同步失败：${syncMessage}`
              : `Notion 自动同步失败：${syncMessage}`;
            return {
              level: "warning",
              message: merged,
              nextStep: "下一步：去“发布中心”重试发布，或在“系统健康”查看失败细节。"
            };
          });
        }
      }
    } catch (e) {
      const message = toUserMessage(e, "更新失败");
      setActionFeedback({
        level: "error",
        message: `提交失败：${message}`,
        nextStep: "下一步：点击“重试加载队列”刷新后再提交；若仍失败，请切换异常标签排查。"
      });
      onNotify({ level: "error", message: `提交失败：${message}` });
    } finally {
      setActionBusy(false);
    }
  };

  const onDecision = async (decision: Decision) => {
    if (!selectedItem) {
      return;
    }
    const idx = visibleItems.findIndex((item) => item.id === selectedItem.id);
    const nextId = visibleItems[idx + 1]?.id ?? visibleItems[idx - 1]?.id ?? null;
    await applyDecision([selectedItem.id], decision, nextId);
  };

  const onBatchDecision = async (decision: Decision) => {
    await applyDecision(checkedIds, decision);
  };

  const onRetry = async () => {
    const ids = checkedIds.length > 0 ? checkedIds : visibleItems.map((item) => item.id);
    if (ids.length === 0) {
      setActionFeedback({
        level: "warning",
        message: "没有可重试的异常项",
        nextStep: "下一步：切换到“异常”标签后选择记录，再执行批量重试。"
      });
      return;
    }
    setActionBusy(true);
    setActionFeedback(null);
    try {
      const result = await retryFailedItems(ids);
      const level = result.requeued === result.requested ? "success" : "warning";
      setActionFeedback({
        level,
        message: `重试完成：${result.requeued}/${result.requested}`,
        nextStep:
          result.requeued === result.requested
            ? "下一步：回到“待审核”标签继续处理。"
            : "下一步：检查失败明细并再次批量重试。"
      });
      onNotify({ level, message: `重试完成：${result.requeued}/${result.requested}` });
      appendSessionLog("批量重试", `重试 ${result.requested} 条，成功重排 ${result.requeued} 条`);
      await reload();
    } catch (e) {
      const message = toUserMessage(e, "重试失败");
      setActionFeedback({
        level: "error",
        message: `重试失败：${message}`,
        nextStep: "下一步：先刷新队列，再确认失败记录仍在异常列表后重试。"
      });
      onNotify({ level: "error", message: `重试失败：${message}` });
    } finally {
      setActionBusy(false);
    }
  };

  return (
    <section className="review-layout">
      <div className="review-main">
        <header className="page-header">
          <h1>审核队列</h1>
          <p>审核只处理“异常/低置信”内容；审核提交后会自动触发一次 Notion 同步并回显结果。</p>
        </header>

        <ReviewTabs tab={tab} tabCounts={tabCounts} onTabChange={setTab} />

        <ReviewExceptionFilters
          enabled={tab === "exception"}
          reasonStats={reasonStats}
          reasonFilter={reasonFilter}
          onReasonFilterChange={setReasonFilter}
          priorityFilter={priorityFilter}
          onPriorityFilterChange={setPriorityFilter}
        />

        {tab !== "done" && (
          <ReviewSessionPanel
            processedCount={processedCount}
            throughput={throughput}
            projected15m={projected15m}
            targetProgress={targetProgress}
            targetGap={targetGap}
            targetReached={targetReached}
            sessionLogs={sessionLogs}
            lastFailures={lastFailures}
            onClearFailures={() => setLastFailures([])}
          />
        )}

        {tab === "exception" && (
          <div className="inline-actions">
            <button
              type="button"
              className="button-primary"
              onClick={onRetry}
              disabled={actionBusy || visibleItems.length === 0}
              data-testid="review-batch-retry"
            >
              {actionBusy ? "重试中..." : "批量重试"}
            </button>
            <span className="hint">默认重试选中项；未选中则重试当前列表全部。</span>
          </div>
        )}

        {actionBusy && <div className="state-panel">正在提交审核操作，请等待完成。</div>}

        {actionFeedback && (
          <div
            className={`state-panel ${actionFeedback.level === "error" ? "error" : actionFeedback.level === "warning" ? "warning" : ""}`}
            role={actionFeedback.level === "error" ? "alert" : "status"}
            aria-live={actionFeedback.level === "error" ? "assertive" : "polite"}
            data-testid="review-action-feedback"
          >
            <div>{actionFeedback.message}</div>
            {actionFeedback.nextStep && <small>{actionFeedback.nextStep}</small>}
            <div className="inline-actions">
              <button
                type="button"
                className="button-secondary"
                onClick={() => reload()}
                disabled={actionBusy}
                data-testid="review-refresh"
              >
                刷新队列
              </button>
              {tab === "exception" && (
                <button
                  type="button"
                  className="button-secondary"
                  onClick={onRetry}
                  disabled={actionBusy || visibleItems.length === 0}
                  data-testid="review-retry-again"
                >
                  再次重试
                </button>
              )}
            </div>
          </div>
        )}

        <div className="filter-bar">
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="按标题/原因/ID筛选" />
          <select value={sortMode} onChange={(event) => setSortMode(event.target.value as SortMode)}>
            <option value="priority_desc">优先级：高到低</option>
            <option value="priority_asc">优先级：低到高</option>
          </select>
          <span className="hint">当前显示 {visibleItems.length} 条</span>
        </div>

        {checkedIds.length > 0 && (
          <div className="bulk-actions">
            <span>已选 {checkedIds.length} 条</span>
            <button
              type="button"
              className="button-primary"
              onClick={() => onBatchDecision("approved")}
              disabled={actionBusy}
            >
              批量通过
            </button>
            <button
              type="button"
              className="button-secondary"
              onClick={() => onBatchDecision("rejected")}
              disabled={actionBusy}
            >
              批量驳回
            </button>
            <button
              type="button"
              className="button-secondary"
              onClick={() => onBatchDecision("duplicate")}
              disabled={actionBusy}
            >
              批量标记重复
            </button>
            <button
              type="button"
              className="button-secondary"
              onClick={() => setCheckedIds([])}
              disabled={actionBusy}
            >
              清空选择
            </button>
          </div>
        )}

        {!loading && !error && items.length > 0 && visibleItems.length === 0 && (
          <div className="state-panel">
            当前筛选条件没有结果。
            <div className="inline-actions">
              <button
                type="button"
                className="button-secondary"
                onClick={() => {
                  setQuery("");
                  setSortMode("priority_desc");
                  setReasonFilter("all");
                  setPriorityFilter("all");
                }}
              >
                清空筛选条件
              </button>
            </div>
          </div>
        )}

        <RemoteState
          loading={loading}
          error={error}
          empty={items.length === 0}
          loadingText="审核队列加载中..."
          emptyText={
            tab === "exception"
              ? "当前没有异常项（通常表示暂未驳回或重试失败）。"
              : tab === "done"
                ? "当前没有已完成项（请先在待审核页执行通过/驳回）。"
                : "当前没有待审核项。"
          }
          errorNextStep="下一步：点击“重试加载队列”；若仍失败，请切到“系统健康”查看同步失败记录。"
          emptyNextStep={
            tab === "exception"
              ? "下一步：返回“待审核”继续处理，或先在系统健康页触发失败回放。"
              : tab === "done"
                ? "下一步：切换到“待审核”处理新数据，或到收件箱抓取新内容。"
                : "下一步：先去收件箱抓取新内容，再回到此处审核。"
          }
          onRetry={() => void reload()}
          retryLabel="重试加载队列"
        >
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>
                    <input
                      type="checkbox"
                      checked={allVisibleChecked}
                      onChange={(event) => {
                        if (event.target.checked) {
                          setCheckedIds(visibleItems.map((item) => item.id));
                        } else {
                          setCheckedIds([]);
                        }
                      }}
                    />
                  </th>
                  <th>标题</th>
                  <th>优先级</th>
                  <th>状态</th>
                  <th>原因</th>
                </tr>
              </thead>
              <tbody>
                {visibleItems.map((item) => (
                  <tr
                    key={item.id}
                    className={selectedId === item.id ? "clickable-row active" : "clickable-row"}
                    onClick={() => setSelectedId(item.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        setSelectedId(item.id);
                      }
                    }}
                    tabIndex={0}
                  >
                    <td>
                      <input
                        type="checkbox"
                        checked={checkedIds.includes(item.id)}
                        onChange={(event) => {
                          event.stopPropagation();
                          setCheckedIds((prev) =>
                            event.target.checked ? [...prev, item.id] : prev.filter((id) => id !== item.id)
                          );
                        }}
                      />
                    </td>
                    <td>
                      <div className="title-cell">
                        <span>{item.title}</span>
                        <small>{item.normalizedId.slice(0, 10)}</small>
                      </div>
                    </td>
                    <td>{item.priority.toFixed(2)}</td>
                    <td>
                      <span className={`tag ${reviewStateTone(item.state)}`}>{formatReviewState(item.state)}</span>
                    </td>
                    <td>{item.reason}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </RemoteState>
      </div>

      <ReviewDrawer item={selectedItem} busy={actionBusy} onClose={() => setSelectedId(null)} onDecision={onDecision} />
    </section>
  );
}
