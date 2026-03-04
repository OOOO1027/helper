import { RemoteState } from "../../components/RemoteState";
import type { SyncLog } from "../../types/contracts";
import { formatTime } from "../../utils/time";
import { formatLogState, logStateTone } from "./settingsHelpers";
import type { LogStateFilter, RangeDays } from "./settingsHelpers";

interface Props {
  rangeHint: string;
  logs: SyncLog[];
  filteredLogs: SyncLog[];
  logStateStats: [string, number][];
  logsLoading: boolean;
  logsError: string | null;
  logsPage: number;
  logsTotalPages: number;
  logsTotal: number;
  logsStateFilter: LogStateFilter;
  rangeDays: RangeDays;
  actionBusy: boolean;
  hasActiveFilters: boolean;
  onRangeDaysChange: (v: RangeDays) => void;
  onLogsStateFilterChange: (v: LogStateFilter) => void;
  onResetFilters: () => void;
  onPrevPage: () => void;
  onNextPage: () => void;
  onRefresh: () => void;
  onQueueReplayId: (id: string) => void;
}

export function SyncLogsPanel({
  rangeHint,
  logs,
  filteredLogs,
  logStateStats,
  logsLoading,
  logsError,
  logsPage,
  logsTotalPages,
  logsTotal,
  logsStateFilter,
  rangeDays,
  actionBusy,
  hasActiveFilters,
  onRangeDaysChange,
  onLogsStateFilterChange,
  onResetFilters,
  onPrevPage,
  onNextPage,
  onRefresh,
  onQueueReplayId,
}: Props) {
  return (
    <article className="panel-card">
      <h3>同步日志（{rangeHint}）</h3>
      <div className="filter-bar">
        <select
          value={rangeDays}
          onChange={(event) => {
            onRangeDaysChange(Number(event.target.value) as RangeDays);
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
          onChange={(event) => onLogsStateFilterChange(event.target.value as LogStateFilter)}
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
        <button type="button" onClick={onResetFilters} disabled={actionBusy || !hasActiveFilters}>
          重置筛选
        </button>
      </div>
      {logStateStats.length > 0 && (
        <div className="reason-bar">
          <button
            type="button"
            className={logsStateFilter === "all" ? "reason-chip active" : "reason-chip"}
            onClick={() => onLogsStateFilterChange("all")}
            disabled={actionBusy}
          >
            全部 ({logs.length})
          </button>
          {logStateStats.map(([state, count]) => (
            <button
              key={state}
              type="button"
              className={logsStateFilter === state ? "reason-chip active" : "reason-chip"}
              onClick={() => onLogsStateFilterChange(state as LogStateFilter)}
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
        errorNextStep="下一步：点击「重试加载日志」；若仍失败，请先执行一次「立即同步一次」再刷新。"
        emptyNextStep="下一步：先将状态切到「全部」或扩大时间范围，再点击「刷新日志」。"
        onRetry={onRefresh}
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
          onClick={onPrevPage}
          disabled={actionBusy || logsLoading || logsPage <= 1}
        >
          上一页
        </button>
        <span className="hint">
          日志页 {logsPage}/{logsTotalPages} · 共 {logsTotal} 条
        </span>
        <button
          type="button"
          onClick={onNextPage}
          disabled={actionBusy || logsLoading || logsPage >= logsTotalPages}
        >
          下一页
        </button>
        <button
          type="button"
          onClick={onRefresh}
          disabled={actionBusy || logsLoading}
          data-testid="settings-refresh-logs"
        >
          刷新日志
        </button>
      </div>
    </article>
  );
}
