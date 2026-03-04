import type { PriorityFilter } from "./types";

interface Props {
  enabled: boolean;
  reasonStats: [string, number][];
  reasonFilter: string;
  onReasonFilterChange: (value: string) => void;
  priorityFilter: PriorityFilter;
  onPriorityFilterChange: (value: PriorityFilter) => void;
}

export function ReviewExceptionFilters({
  enabled,
  reasonStats,
  reasonFilter,
  onReasonFilterChange,
  priorityFilter,
  onPriorityFilterChange
}: Props) {
  if (!enabled) {
    return null;
  }

  return (
    <>
      {reasonStats.length > 0 && (
        <div className="reason-bar">
          <button
            type="button"
            className={reasonFilter === "all" ? "reason-chip active" : "reason-chip"}
            onClick={() => onReasonFilterChange("all")}
          >
            全部
          </button>
          {reasonStats.map(([reason, count]) => (
            <button
              key={reason}
              type="button"
              className={reasonFilter === reason ? "reason-chip active" : "reason-chip"}
              onClick={() => onReasonFilterChange(reason)}
              title={reason}
            >
              {reason.slice(0, 24)} ({count})
            </button>
          ))}
        </div>
      )}

      <div className="reason-bar">
        <button
          type="button"
          className={priorityFilter === "all" ? "reason-chip active" : "reason-chip"}
          onClick={() => onPriorityFilterChange("all")}
        >
          优先级：全部
        </button>
        <button
          type="button"
          className={priorityFilter === "high" ? "reason-chip active" : "reason-chip"}
          onClick={() => onPriorityFilterChange("high")}
        >
          高优先级
        </button>
        <button
          type="button"
          className={priorityFilter === "mid" ? "reason-chip active" : "reason-chip"}
          onClick={() => onPriorityFilterChange("mid")}
        >
          中优先级
        </button>
        <button
          type="button"
          className={priorityFilter === "low" ? "reason-chip active" : "reason-chip"}
          onClick={() => onPriorityFilterChange("low")}
        >
          低优先级
        </button>
      </div>
    </>
  );
}
