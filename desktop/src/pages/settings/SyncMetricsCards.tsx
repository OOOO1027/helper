import { formatTime } from "../../utils/time";
import type { LatestImportSnapshot } from "./settingsHelpers";

interface SyncHeadline {
  text: string;
  at: string;
}

interface DailyOverview {
  totalRuns: number;
  successRate: number;
  avgSuccessPerRun: number;
  dayCount: number;
  jobTypeCount: number;
  topJobType: string;
}

interface Props {
  latestImportSnapshot: LatestImportSnapshot | null;
  latestSyncHeadline: SyncHeadline | null;
  failedCount: number;
  replayableCandidateCount: number;
  dailyOverview: DailyOverview | null;
}

export function SyncMetricsCards({
  latestImportSnapshot,
  latestSyncHeadline,
  failedCount,
  replayableCandidateCount,
  dailyOverview,
}: Props) {
  return (
    <>
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
              <small>点击"立即同步一次"开始同步</small>
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
    </>
  );
}
