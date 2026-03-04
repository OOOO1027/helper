import type { ReviewItem, ReviewTab } from "../../types/contracts";

export type SortMode = "priority_desc" | "priority_asc";
export type Decision = "approved" | "rejected" | "duplicate";
export type PriorityFilter = "all" | "high" | "mid" | "low";

export type TabCounts = Record<ReviewTab, number>;

export interface SessionActionLog {
  id: string;
  at: number;
  action: string;
  detail: string;
}

export interface BatchFailure {
  id: string;
  message: string;
}

export interface ActionFeedback {
  level: "success" | "warning" | "error";
  message: string;
  nextStep?: string;
}

export interface ReviewToneFormatter {
  formatReviewState: (state: string) => string;
  reviewStateTone: (state: string) => "success" | "warning" | "error" | "neutral";
  priorityTone: (priority: number) => "low" | "mid" | "high";
}

export interface ReviewSelection {
  items: ReviewItem[];
  checkedIds: string[];
}
