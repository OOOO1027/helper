import type { ReviewTab } from "../../types/contracts";
import type { TabCounts } from "./types";

interface Props {
  tab: ReviewTab;
  tabCounts: TabCounts;
  onTabChange: (tab: ReviewTab) => void;
}

export function ReviewTabs({ tab, tabCounts, onTabChange }: Props) {
  return (
    <div className="tabs">
      <button
        type="button"
        className={tab === "pending" ? "active" : ""}
        onClick={() => onTabChange("pending")}
        data-testid="review-tab-pending"
      >
        待审核
        <span className="tab-count">{tabCounts.pending}</span>
      </button>
      <button
        type="button"
        className={tab === "exception" ? "active" : ""}
        onClick={() => onTabChange("exception")}
        data-testid="review-tab-exception"
      >
        异常
        <span className="tab-count">{tabCounts.exception}</span>
      </button>
      <button
        type="button"
        className={tab === "done" ? "active" : ""}
        onClick={() => onTabChange("done")}
        data-testid="review-tab-done"
      >
        已完成
        <span className="tab-count">{tabCounts.done}</span>
      </button>
    </div>
  );
}
