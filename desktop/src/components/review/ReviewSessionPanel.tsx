import type { BatchFailure, SessionActionLog } from "./types";

interface Props {
  processedCount: number;
  throughput: number;
  projected15m: number;
  targetProgress: number;
  targetGap: number;
  targetReached: boolean;
  sessionLogs: SessionActionLog[];
  lastFailures: BatchFailure[];
  onClearFailures: () => void;
}

export function ReviewSessionPanel({
  processedCount,
  throughput,
  projected15m,
  targetProgress,
  targetGap,
  targetReached,
  sessionLogs,
  lastFailures,
  onClearFailures,
}: Props) {
  return (
    <>
      <section className="session-panel">
        <div className="session-stat">
          <span>本次处理</span>
          <strong>{processedCount} 条</strong>
        </div>
        <div className="session-stat">
          <span>当前速率</span>
          <strong>{throughput.toFixed(1)} 条/分钟</strong>
        </div>
        <div className="session-stat">
          <span>15分钟预估</span>
          <strong>{projected15m} 条</strong>
        </div>
        <div className="session-target">
          <span>目标进度（90条）</span>
          <div className="target-track">
            <div className="target-fill" style={{ width: `${targetProgress}%` }} />
          </div>
          <div className="target-meta">
            <span className={targetReached ? "target-badge ok" : "target-badge"}>
              {targetReached ? "达标" : "冲刺中"}
            </span>
            <small>{targetReached ? "已达到日常审核目标" : `距离目标还差 ${targetGap} 条`}</small>
          </div>
        </div>
      </section>

      {sessionLogs.length > 0 && (
        <section className="session-log">
          <h3>最近操作</h3>
          <ul>
            {sessionLogs.map((log) => (
              <li key={log.id}>
                <span>{new Date(log.at).toLocaleTimeString("zh-CN")}</span>
                <strong>{log.action}</strong>
                <small>{log.detail}</small>
              </li>
            ))}
          </ul>
        </section>
      )}

      {lastFailures.length > 0 && (
        <section className="batch-failures">
          <header>
            <h3>批量失败明细</h3>
            <button type="button" onClick={onClearFailures}>
              清空
            </button>
          </header>
          <ul>
            {lastFailures.map((item) => (
              <li key={`${item.id}-${item.message}`}>
                <code>{item.id.slice(0, 8)}</code>
                <span>{item.message}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </>
  );
}
