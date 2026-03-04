import type { SyncRunSummary } from "../../types/contracts";

interface Props {
  biliEnabled: boolean;
  onBiliEnabledChange: (v: boolean) => void;
  syncLimitInput: string;
  onSyncLimitChange: (v: string) => void;
  onManualSync: () => void;
  syncing: boolean;
  actionBusy: boolean;
  lastSyncSummary: SyncRunSummary | null;
  deadLetterIds: string;
  onDeadLetterIdsChange: (v: string) => void;
  onClearDeadLetterIds: () => void;
  onRetryDeadLetters: () => void;
  retryingDeadLetters: boolean;
  deadLetterIdCount: number;
}

export function SyncActionsPanel({
  biliEnabled,
  onBiliEnabledChange,
  syncLimitInput,
  onSyncLimitChange,
  onManualSync,
  syncing,
  actionBusy,
  lastSyncSummary,
  deadLetterIds,
  onDeadLetterIdsChange,
  onClearDeadLetterIds,
  onRetryDeadLetters,
  retryingDeadLetters,
  deadLetterIdCount
}: Props) {
  return (
    <div className="settings-grid">
      <article className="panel-card">
        <h3>来源设置</h3>
        <label className="switch-row">
          <span>B站自动抓取</span>
          <input
            type="checkbox"
            checked={biliEnabled}
            onChange={(e) => onBiliEnabledChange(e.target.checked)}
          />
          <small>{biliEnabled ? "已开启" : "默认关闭"}</small>
        </label>
        <p className="hint">当前默认关闭，避免无意产生额外采集负载。该开关仅在本机生效。</p>
      </article>

      <article className="panel-card">
        <h3>Notion 同步提示</h3>
        <p>展示层状态：已连接/可同步/失败可重试。</p>
        <p className="hint">不展示 token、字段映射等技术细节。</p>
        <input
          className="text-input"
          type="number"
          min={1}
          step={1}
          value={syncLimitInput}
          onChange={(event) => onSyncLimitChange(event.target.value)}
          placeholder="同步上限（默认 50）"
          disabled={actionBusy}
        />
        <div className="inline-actions">
          <button
            type="button"
            className="button-primary"
            onClick={onManualSync}
            disabled={actionBusy}
            data-testid="settings-manual-sync"
          >
            {syncing ? "同步中..." : "立即同步一次"}
          </button>
          <span className="hint">本次仅执行一轮同步，不修改常驻策略。</span>
        </div>
        {actionBusy && (
          <div className="state-panel" role="status" aria-live="polite">
            {syncing ? "正在执行手动同步，请等待结果返回。" : "正在执行失败回放，请等待结果返回。"}
          </div>
        )}
        {lastSyncSummary && (
          <div className="state-panel">
            最近手动同步：扫描 {lastSyncSummary.scanned}，尝试 {lastSyncSummary.attempted}，成功{" "}
            {lastSyncSummary.succeeded}，失败 {lastSyncSummary.failed}。
          </div>
        )}
      </article>

      <article className="panel-card">
        <h3>失败回放</h3>
        <p className="hint">输入 dead letter 项目 ID（逗号或空格分隔），可手动回放入队。</p>
        <input
          className="text-input"
          value={deadLetterIds}
          onChange={(event) => onDeadLetterIdsChange(event.target.value)}
          placeholder="例如：dl_123, dl_456"
          disabled={actionBusy}
          data-testid="settings-dead-letter-ids"
        />
        <div className="inline-actions">
          <button
            type="button"
            onClick={onClearDeadLetterIds}
            disabled={actionBusy || deadLetterIds.trim().length === 0}
            data-testid="settings-clear-dead-letter-ids"
          >
            清空 ID
          </button>
          <button
            type="button"
            className="button-primary"
            onClick={onRetryDeadLetters}
            disabled={actionBusy}
            data-testid="settings-retry-dead-letters"
          >
            {retryingDeadLetters ? "回放中..." : "回放失败项"}
          </button>
          <span className="hint">已填入 {deadLetterIdCount} 个 ID，不会覆盖历史记录，仅重排待处理队列。</span>
        </div>
      </article>
    </div>
  );
}
