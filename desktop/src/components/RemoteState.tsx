import type { ReactNode } from "react";

interface Props {
  loading: boolean;
  error: string | null;
  empty: boolean;
  loadingText?: string;
  emptyText?: string;
  errorNextStep?: string;
  emptyNextStep?: string;
  onRetry?: () => void;
  retryLabel?: string;
  children: ReactNode;
}

export function RemoteState(props: Props) {
  const {
    loading,
    error,
    empty,
    loadingText = "加载中...",
    emptyText = "暂无数据",
    errorNextStep,
    emptyNextStep,
    onRetry,
    retryLabel = "重试",
    children
  } = props;

  if (loading) {
    return (
      <div className="state-panel" role="status" aria-live="polite">
        {loadingText}
      </div>
    );
  }
  if (error) {
    return (
      <div className="state-panel error" role="alert" aria-live="assertive">
        <div>{error}</div>
        {errorNextStep && <small>{errorNextStep}</small>}
        {onRetry && (
          <div className="inline-actions">
            <button type="button" onClick={onRetry}>
              {retryLabel}
            </button>
          </div>
        )}
      </div>
    );
  }
  if (empty) {
    return (
      <div className="state-panel empty" role="status" aria-live="polite">
        <div>{emptyText}</div>
        {emptyNextStep && <small>{emptyNextStep}</small>}
      </div>
    );
  }
  return <>{children}</>;
}
