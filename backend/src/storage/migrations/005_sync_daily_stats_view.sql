CREATE VIEW IF NOT EXISTS v_sync_daily_stats AS
SELECT
  substr(created_at, 1, 10) AS day,
  job_type,
  COUNT(1) AS run_count,
  SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) AS success_runs,
  SUM(CASE WHEN status = 'partial' THEN 1 ELSE 0 END) AS partial_runs,
  SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_runs,
  ROUND(
    1.0 * SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) / NULLIF(COUNT(1), 0),
    4
  ) AS success_rate,
  ROUND(AVG(success_count), 4) AS avg_success_count,
  ROUND(AVG(fail_count), 4) AS avg_fail_count
FROM job_runs
WHERE job_type IN ('notion_smoke', 'notion_sync_once')
GROUP BY substr(created_at, 1, 10), job_type;
