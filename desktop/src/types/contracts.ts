export interface ApiError {
  code: string;
  message: string;
  retryable: boolean;
}

export type ApiResponse<T> = {
  ok: boolean;
  request_id: string;
  data?: T;
  error?: ApiError;
};

export interface PageReq {
  page: number;
  page_size: number;
}

export interface Paged<T> {
  items: T[];
  page: number;
  page_size: number;
  total: number;
}

export interface DashboardMetrics {
  today_collected: number;
  pending_review: number;
  classification_accuracy: number;
  summary_usability: number;
  budget_usage_ratio: number;
}

export interface CollectionSourceBreakdown {
  source: string;
  count: number;
}

export interface CollectionRecentItem {
  source: string;
  title: string;
  collected_at: string;
  review_state: string;
  sync_state: string;
}

export interface CollectionInsight {
  day: string;
  today_total: number;
  sources: CollectionSourceBreakdown[];
  review_pending: number;
  review_done: number;
  review_rejected: number;
  direct_no_review: number;
  sync_pending: number;
  sync_success: number;
  sync_failed: number;
  sync_retry: number;
  sync_not_started: number;
  recent_items: CollectionRecentItem[];
}

export interface BudgetStatus {
  month: string;
  limit_cny: number;
  used_cny: number;
}

export interface AiUsageModelItem {
  model: string;
  calls: number;
  tokens_in: number;
  tokens_out: number;
  cost_cny: number;
}

export interface AiUsageDailyItem {
  day: string;
  calls: number;
  tokens_in: number;
  tokens_out: number;
  cost_cny: number;
}

export interface AiUsageMonthlySummary {
  month: string;
  budget_limit_cny: number;
  used_cny: number;
  usage_ratio: number;
  remaining_cny: number;
  calls: number;
  tokens_in: number;
  tokens_out: number;
  avg_cost_per_call_cny: number;
  models: AiUsageModelItem[];
  daily: AiUsageDailyItem[];
}

export interface ImportPersistSummary {
  batch_id: string;
  total: number;
  parsed_ok: number;
  duplicates: number;
  parse_failed: number;
  persisted: number;
  persist_failed: number;
}

export interface CollectResult {
  source: string;
  fetched: number;
  stored: number;
}

export interface ReviewFilter {
  state?: string;
  min_priority?: number;
}

export interface GetReviewItemsReq {
  filter: ReviewFilter;
  page: PageReq;
}

export interface GetIngestQueueReq {
  page: PageReq;
}

export interface GetReviewQueueReq {
  filter: ReviewFilter;
  page: PageReq;
}

export interface ReviewPatch {
  state?: string;
  note?: string;
}

export interface UpdateReviewItemReq {
  id: string;
  patch: ReviewPatch;
}

export interface RetryResult {
  requested: number;
  requeued: number;
  ignored: number;
}

export interface SyncRunSummary {
  scanned: number;
  attempted: number;
  succeeded: number;
  failed: number;
  requeued: number;
  dead_lettered: number;
}

export interface PublishResult {
  request_id: string;
  attempted: number;
  succeeded: number;
  failed: number;
  requeued: number;
  dead_lettered: number;
}

export interface SyncConfigSnapshot {
  mode: "database" | "page_tree" | "unknown";
  token_configured: boolean | null;
  database_id_configured: boolean | null;
  root_page_id_configured: boolean | null;
  timezone: string | null;
  category_growth_limit: number | null;
  source: "ipc" | "fallback";
}

export interface LocalToolStatus {
  textutil: boolean;
  pdftotext: boolean;
}

export interface DateRange {
  from: string;
  to: string;
}

export interface SyncLog {
  id: string;
  state: string;
  error_code?: string | null;
  created_at: string;
}

export interface SyncDailyStat {
  day: string;
  job_type: string;
  run_count: number;
  success_runs: number;
  partial_runs: number;
  failed_runs: number;
  success_rate: number;
  avg_success_count: number;
  avg_fail_count: number;
}

export interface BackendReviewItem {
  id: string;
  normalized_item_id: string;
  priority: number;
  quality_band: string;
  reason: string;
  state: string;
  publish_state: string;
  title?: string;
  url?: string;
  conclusion_summary?: string | null;
  summary?: string | null;
  key_points?: string[] | string | null;
  key_points_json?: string | null;
  tags?: string[] | string | null;
  tags_json?: string | null;
  cover_url?: string | null;
  image_urls?: string[] | string | null;
  image_urls_json?: string | null;
}

export interface IngestQueueItem {
  source_item_id: string;
  source: string;
  title: string;
  collected_at: string;
  review_state: string;
  publish_state: string;
}

export interface PublishTask {
  id: string;
  item_id: string;
  title: string;
  state: string;
  attempt_count: number;
  next_retry_at?: string | null;
  error_code?: string | null;
  updated_at: string;
}

export interface PublishHistoryItem {
  id: string;
  state: string;
  error_code?: string | null;
  created_at: string;
  success_count: number;
  fail_count: number;
  pipeline_stage?: string | null;
  sync_mode?: string | null;
  retryable?: boolean | null;
  category?: string | null;
  week_key?: string | null;
  route_reason?: string | null;
  conclusion_summary?: string | null;
  key_points?: string[] | null;
  tags?: string[] | null;
  cover_url?: string | null;
  image_urls?: string[] | null;
  quality_state?: string | null;
  quality_score?: number | null;
  degraded_fields?: string[] | null;
}

export interface PublishHistoryFilter {
  state?: string;
  sync_mode?: string;
  retryable?: boolean;
}

export interface SourceStateMarkResult {
  item_id: string;
  active_state: string;
  updated_at: string;
}

export type ReviewTab = "pending" | "exception" | "done";

export interface ReviewItem {
  id: string;
  normalizedId: string;
  title: string;
  url?: string;
  priority: number;
  qualityBand: string;
  reason: string;
  state: string;
  publishState: string;
  conclusionSummary?: string | null;
  keyPoints?: string[];
  tags?: string[];
  coverUrl?: string | null;
  imageUrls?: string[];
  tab: ReviewTab;
}
