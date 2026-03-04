import { RemoteState } from "../../components/RemoteState";
import type { SyncDailyStat } from "../../types/contracts";
import { formatJobType, formatLogState } from "./settingsHelpers";
import type { DailyJobTypeFilter, LogStateFilter } from "./settingsHelpers";

interface Props {
  rangeHint: string;
  dailyStats: SyncDailyStat[];
  statsLoading: boolean;
  statsError: string | null;
  statsPage: number;
  statsTotalPages: number;
  statsTotal: number;
  dailyJobTypeFilter: DailyJobTypeFilter;
  dailyStatusFilter: LogStateFilter;
  dailyStatusCustomInput: string;
  dailyStatusOverride: string | null;
  actionBusy: boolean;
  onJobTypeFilterChange: (v: DailyJobTypeFilter) => void;
  onStatusFilterChange: (v: LogStateFilter) => void;
  onCustomStatusInput: (v: string) => void;
  onApplyCustomStatus: () => void;
  onPrevPage: () => void;
  onNextPage: () => void;
  onRefresh: () => void;
}

export function DailyStatsPanel({
  rangeHint,
  dailyStats,
  statsLoading,
  statsError,
  statsPage,
  statsTotalPages,
  statsTotal,
  dailyJobTypeFilter,
  dailyStatusFilter,
  dailyStatusCustomInput,
  dailyStatusOverride,
  actionBusy,
  onJobTypeFilterChange,
  onStatusFilterChange,
  onCustomStatusInput,
  onApplyCustomStatus,
  onPrevPage,
  onNextPage,
  onRefresh
}: Props) {
  return (
    <article className="panel-card">
      <h3>日统计（{rangeHint}）</h3>
      <div className="filter-bar">
        <select
          value={dailyJobTypeFilter}
          onChange={(event) => {
            onJobTypeFilterChange(event.target.value as DailyJobTypeFilter);
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
            onStatusFilterChange(
              event.target.value as LogStateFilter
            );
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
          onChange={(event) => onCustomStatusInput(event.target.value)}
          placeholder="高级：自定义运行状态（可输入非法值做验收）"
          disabled={actionBusy}
          data-testid="settings-custom-daily-status"
        />
        <button
          type="button"
          onClick={onApplyCustomStatus}
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
        errorNextStep="下一步：点击"重试加载统计"；若仍失败，请先刷新日志确认同步记录是否存在。"
        emptyNextStep="下一步：切换任务类型为"全部"并放宽时间范围，再刷新统计。"
        onRetry={onRefresh}
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
          onClick={onPrevPage}
          disabled={actionBusy || statsLoading || statsPage <= 1}
        >
          上一页
        </button>
        <span className="hint">
          统计页 {statsPage}/{statsTotalPages} · 共 {statsTotal} 条
        </span>
        <button
          type="button"
          onClick={onNextPage}
          disabled={actionBusy || statsLoading || statsPage >= statsTotalPages}
        >
          下一页
        </button>
        <button
          type="button"
          onClick={onRefresh}
          disabled={actionBusy || statsLoading}
          data-testid="settings-refresh-stats"
        >
          刷新统计
        </button>
      </div>
    </article>
  );
}
