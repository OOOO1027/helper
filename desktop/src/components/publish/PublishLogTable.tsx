import { Fragment, useId, useMemo, useState } from "react";
import type { PublishHistoryItem } from "../../types/contracts";

export type PublishSyncModeFilter = "all" | "page_tree" | "database" | "unknown";

interface Props {
  logs: PublishHistoryItem[];
  showRetryableOnly: boolean;
  onShowRetryableOnlyChange: (value: boolean) => void;
  syncModeFilter: PublishSyncModeFilter;
  onSyncModeFilterChange: (value: PublishSyncModeFilter) => void;
  emptyText?: string;
  emptyNextStep?: string;
  tableAriaLabel?: string;
}

function formatState(state: string): string {
  if (state === "success") {
    return "成功";
  }
  if (state === "failed") {
    return "失败";
  }
  if (state === "partial") {
    return "部分成功";
  }
  if (state === "started") {
    return "进行中";
  }
  if (state === "retry") {
    return "重试中";
  }
  return state;
}

function stateTone(state: string): "success" | "error" | "warning" | "neutral" {
  if (state === "success") {
    return "success";
  }
  if (state === "failed") {
    return "error";
  }
  if (state === "partial" || state === "started" || state === "retry") {
    return "warning";
  }
  return "neutral";
}

function formatRetryable(retryable?: boolean | null): string {
  if (retryable === undefined || retryable === null) {
    return "-";
  }
  return retryable ? "是" : "否";
}

function formatExecutionPath(item: PublishHistoryItem): string {
  const mode = item.sync_mode ?? "-";
  const stage = item.pipeline_stage ?? "-";
  if (mode === "-" && stage === "-") {
    return "-";
  }
  return `${mode} / ${stage}`;
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "-";
  }
  return date.toLocaleString("zh-CN");
}

function normalizeMode(mode?: string | null): string {
  if (!mode) {
    return "unknown";
  }
  const normalized = mode.trim().toLowerCase();
  if (!normalized) {
    return "unknown";
  }
  return normalized;
}

function formatRouteValue(value?: string | null): string {
  const normalized = value?.trim();
  return normalized && normalized.length > 0 ? normalized : "-";
}

function parseList(raw: unknown): string[] {
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
        // Fallback to split below.
      }
    }
    return normalized
      .split(/[\n,，;；|]/)
      .map((item) => item.trim())
      .filter((item) => item.length > 0);
  }
  return [];
}

function summarizeFailureAction(item: PublishHistoryItem): string {
  if (item.retryable === true) {
    return "建议动作：可在系统健康页执行失败回放，然后刷新日志确认状态变化。";
  }
  if (item.error_code && item.error_code.includes("NOTION")) {
    return "建议动作：先补齐 Notion 配置（凭证/Root Page/模式）后再发起发布。";
  }
  return "建议动作：先检查错误码对应配置与网络状态，再重试发布。";
}

export function PublishLogTable({
  logs,
  showRetryableOnly,
  onShowRetryableOnlyChange,
  syncModeFilter,
  onSyncModeFilterChange,
  emptyText = "当前筛选条件下暂无发布日志。",
  emptyNextStep = "下一步：取消“仅看可重试”或切换 sync_mode 后重试。",
  tableAriaLabel = "发布日志列表"
}: Props) {
  const retryableId = useId();
  const syncModeId = useId();
  const [expandedIds, setExpandedIds] = useState<string[]>([]);

  const routeRefItem = useMemo(
    () =>
      logs.find(
        (item) =>
          formatRouteValue(item.category) !== "-" ||
          formatRouteValue(item.week_key) !== "-" ||
          formatRouteValue(item.route_reason) !== "-"
      ) ?? logs[0],
    [logs]
  );

  const routeReason = routeRefItem?.route_reason?.trim() ?? "";
  const routeReasonIsGrowthLimit = routeReason === "growth_limit_exceeded";
  const routeCategory = formatRouteValue(routeRefItem?.category);
  const routeWeek = formatRouteValue(routeRefItem?.week_key);
  const routeReasonDisplay = formatRouteValue(routeRefItem?.route_reason);

  const toggleExpand = (id: string) => {
    setExpandedIds((prev) => (prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]));
  };

  return (
    <>
      <article className="route-explain-card" aria-label="路由解释">
        <div className="route-explain-grid">
          <div className="kv-row">
            <span>category</span>
            <code>{routeCategory}</code>
          </div>
          <div className="kv-row">
            <span>week</span>
            <code>{routeWeek}</code>
          </div>
          <div className="kv-row">
            <span>route_reason</span>
            <code>{routeReasonDisplay}</code>
          </div>
        </div>
        {routeReasonIsGrowthLimit ? (
          <small className="hint">
            发生了什么：分类增长已达上限，内容被路由到兜底分类。下一步：在系统健康确认分类增长上限或补充固定分类后重试发布。
          </small>
        ) : (
          <small className="hint">发生了什么：当前显示最近一次发布路由结果。下一步：可展开日志行查看单条详情。</small>
        )}
      </article>

      <div className="filter-bar publish-log-filters">
        <label className="switch-row" htmlFor={retryableId}>
          <input
            id={retryableId}
            type="checkbox"
            checked={showRetryableOnly}
            onChange={(event) => onShowRetryableOnlyChange(event.target.checked)}
          />
          <span>仅看可重试</span>
        </label>
        <label className="filter-field" htmlFor={syncModeId}>
          <span>sync_mode</span>
          <select
            id={syncModeId}
            value={syncModeFilter}
            onChange={(event) => onSyncModeFilterChange(event.target.value as PublishSyncModeFilter)}
          >
            <option value="all">all</option>
            <option value="page_tree">page_tree</option>
            <option value="database">database</option>
            <option value="unknown">unknown</option>
          </select>
        </label>
        <span className="hint">当前显示 {logs.length} 条</span>
      </div>

      {logs.length === 0 ? (
        <div className="state-panel empty" role="status" aria-live="polite">
          <div>{emptyText}</div>
          <small>{emptyNextStep}</small>
        </div>
      ) : (
        <div className="table-wrap compact">
          <table aria-label={tableAriaLabel}>
            <thead>
              <tr>
                <th scope="col">详情</th>
                <th scope="col">时间</th>
                <th scope="col">状态</th>
                <th scope="col">成功</th>
                <th scope="col">失败</th>
                <th scope="col">错误码</th>
                <th scope="col">模式/阶段</th>
                <th scope="col">可重试</th>
                <th scope="col">ID</th>
              </tr>
            </thead>
            <tbody>
              {logs.map((item) => {
                const expanded = expandedIds.includes(item.id);
                const previewPoints = parseList(item.key_points ?? null);
                const previewTags = parseList(item.tags ?? null);
                const previewImages = parseList(item.image_urls ?? null).slice(0, 4);
                return (
                  <Fragment key={item.id}>
                    <tr>
                      <td>
                        <button
                          type="button"
                          className="table-action-button"
                          aria-expanded={expanded}
                          aria-controls={`publish-log-detail-${item.id}`}
                          onClick={() => toggleExpand(item.id)}
                        >
                          {expanded ? "收起" : "展开"}
                        </button>
                      </td>
                      <td>{formatTime(item.created_at)}</td>
                      <td>
                        <span className={`tag ${stateTone(item.state)}`}>{formatState(item.state)}</span>
                      </td>
                      <td>{item.success_count}</td>
                      <td>{item.fail_count}</td>
                      <td>{item.error_code ?? "-"}</td>
                      <td>{formatExecutionPath(item)}</td>
                      <td>{formatRetryable(item.retryable)}</td>
                      <td>{item.id}</td>
                    </tr>
                    {expanded && (
                      <tr id={`publish-log-detail-${item.id}`} className="publish-log-expanded-row">
                        <td colSpan={9}>
                          <div className="publish-log-detail">
                            <div className="kv-row">
                              <span>结构化摘要</span>
                              <strong>{formatRouteValue(item.conclusion_summary)}</strong>
                            </div>
                            <div className="kv-row">
                              <span>关键要点</span>
                              {previewPoints.length > 0 ? (
                                <ul className="preview-list">
                                  {previewPoints.map((point, index) => (
                                    <li key={`${point}-${index}`}>{point}</li>
                                  ))}
                                </ul>
                              ) : (
                                <small className="muted-inline">
                                  发生了什么：未返回关键要点。下一步：回查审核抽屉补充后再发布。
                                </small>
                              )}
                            </div>
                            <div className="kv-row">
                              <span>标签</span>
                              {previewTags.length > 0 ? (
                                <div className="preview-tags">
                                  {previewTags.map((tag) => (
                                    <span key={tag} className="tag neutral">
                                      {tag}
                                    </span>
                                  ))}
                                </div>
                              ) : (
                                <small className="muted-inline">
                                  发生了什么：未返回标签。下一步：补充至少 1 个标签提高检索能力。
                                </small>
                              )}
                            </div>
                            <div className="kv-row">
                              <span>封面与图片数量</span>
                              <code>{previewImages.length > 0 ? `已返回 ${previewImages.length} 张` : "-"}</code>
                            </div>
                            <div className="kv-row">
                              <span>质量判定</span>
                              <code>
                                {item.quality_state ?? "-"}
                                {typeof item.quality_score === "number"
                                  ? ` (${Math.round(item.quality_score)})`
                                  : ""}
                              </code>
                            </div>
                            <div className="kv-row">
                              <span>降级字段</span>
                              <code>
                                {item.degraded_fields && item.degraded_fields.length > 0
                                  ? item.degraded_fields.join(", ")
                                  : "-"}
                              </code>
                            </div>
                            <div className="route-inline">
                              <span>category: {formatRouteValue(item.category)}</span>
                              <span>week: {formatRouteValue(item.week_key)}</span>
                              <span>route_reason: {formatRouteValue(item.route_reason)}</span>
                            </div>
                            {(item.state === "failed" || item.state === "partial") && (
                              <div className="state-panel warning">
                                <div>
                                  发生了什么：本次发布存在失败记录（错误码：{item.error_code ?? "-"}，可重试：
                                  {formatRetryable(item.retryable)}）。
                                </div>
                                <small>{summarizeFailureAction(item)}</small>
                              </div>
                            )}
                          </div>
                        </td>
                      </tr>
                    )}
                  </Fragment>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
