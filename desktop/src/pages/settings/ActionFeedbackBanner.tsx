import type { ActionFeedback, ActionKind } from "./settingsHelpers";

interface Props {
  actionFeedback: ActionFeedback | null;
  lastActionKind: ActionKind;
  actionBusy: boolean;
  onManualSync: () => void;
  onRetryDeadLetters: () => void;
  onViewFailedLogs: () => void;
}

export function ActionFeedbackBanner({
  actionFeedback,
  lastActionKind,
  actionBusy,
  onManualSync,
  onRetryDeadLetters,
  onViewFailedLogs,
}: Props) {
  if (!actionFeedback) {
    return null;
  }
  return (
    <div
      className={`state-panel ${actionFeedback.level === "error" ? "error" : actionFeedback.level === "warning" ? "warning" : ""}`}
      role={actionFeedback.level === "error" ? "alert" : "status"}
      aria-live={actionFeedback.level === "error" ? "assertive" : "polite"}
      data-testid="settings-action-feedback"
    >
      <div>{actionFeedback.message}</div>
      {actionFeedback.nextStep && <small>{actionFeedback.nextStep}</small>}
      <div className="inline-actions">
        {lastActionKind === "sync" && (
          <button
            type="button"
            onClick={onManualSync}
            disabled={actionBusy}
            data-testid="settings-retry-sync"
          >
            重试同步
          </button>
        )}
        {lastActionKind === "replay" && (
          <button
            type="button"
            onClick={onRetryDeadLetters}
            disabled={actionBusy}
            data-testid="settings-retry-replay"
          >
            重试回放
          </button>
        )}
        <button
          type="button"
          onClick={onViewFailedLogs}
          disabled={actionBusy}
          data-testid="settings-view-failed-logs"
        >
          查看失败日志
        </button>
      </div>
    </div>
  );
}
