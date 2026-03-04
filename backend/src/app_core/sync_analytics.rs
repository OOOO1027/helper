use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::{BackendError, Result};

use super::{AppCore, DateRange, PageReq, Paged, SyncDailyStat, SyncLog};

impl AppCore {
    pub fn get_sync_logs(&self, _range: DateRange, page: PageReq) -> Result<Paged<SyncLog>> {
        if page.page == 0 || page.page_size == 0 {
            return Err(BackendError::Validation(
                "page and page_size must be positive".to_string(),
            ));
        }
        Ok(Paged {
            items: vec![SyncLog {
                id: Uuid::new_v4().to_string(),
                state: "success".to_string(),
                error_code: None,
                created_at: chrono::Local::now().to_rfc3339(),
            }],
            page: page.page,
            page_size: page.page_size,
            total: 1,
        })
    }

    pub fn get_sync_logs_with_conn(
        &self,
        conn: &Connection,
        range: DateRange,
        page: PageReq,
    ) -> Result<Paged<SyncLog>> {
        if page.page == 0 || page.page_size == 0 {
            return Err(BackendError::Validation(
                "page and page_size must be positive".to_string(),
            ));
        }
        if range.from.trim().is_empty() || range.to.trim().is_empty() {
            return Err(BackendError::Validation(
                "range.from and range.to must not be empty".to_string(),
            ));
        }

        let total_sync: i64 = conn.query_row(
            "SELECT COUNT(1) FROM sync_records WHERE created_at >= ?1 AND created_at <= ?2",
            params![&range.from, &range.to],
            |r| r.get(0),
        )?;
        let total_smoke: i64 = conn.query_row(
            "SELECT COUNT(1) FROM job_runs
             WHERE job_type IN ('notion_smoke', 'notion_sync_once')
               AND created_at >= ?1
               AND created_at <= ?2",
            params![&range.from, &range.to],
            |r| r.get(0),
        )?;
        let total = (total_sync + total_smoke).max(0) as u64;
        let page_size = page.page_size as i64;
        let offset = ((page.page - 1) * page.page_size) as i64;

        let mut stmt = conn.prepare(
            "SELECT id, state, error_code, created_at
             FROM (
                SELECT id, sync_state AS state, last_error_code AS error_code, created_at
                FROM sync_records
                WHERE created_at >= ?1 AND created_at <= ?2
                UNION ALL
                SELECT id, status AS state, error_code AS error_code, created_at
                FROM job_runs
                WHERE job_type IN ('notion_smoke', 'notion_sync_once')
                  AND created_at >= ?1
                  AND created_at <= ?2
             ) logs
             ORDER BY created_at DESC, id DESC
             LIMIT ?3 OFFSET ?4",
        )?;

        let mapped = stmt.query_map(params![&range.from, &range.to, page_size, offset], |row| {
            Ok(SyncLog {
                id: row.get(0)?,
                state: row.get(1)?,
                error_code: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;

        let mut items = Vec::new();
        for item in mapped {
            items.push(item?);
        }

        Ok(Paged {
            items,
            page: page.page,
            page_size: page.page_size,
            total,
        })
    }

    pub fn get_sync_daily_stats_with_conn(
        &self,
        conn: &Connection,
        range: DateRange,
        page: PageReq,
        job_type: Option<String>,
        status: Option<String>,
    ) -> Result<Paged<SyncDailyStat>> {
        if page.page == 0 || page.page_size == 0 {
            return Err(BackendError::Validation(
                "page and page_size must be positive".to_string(),
            ));
        }
        if range.from.trim().is_empty() || range.to.trim().is_empty() {
            return Err(BackendError::Validation(
                "range.from and range.to must not be empty".to_string(),
            ));
        }
        let from_day = range
            .from
            .get(0..10)
            .ok_or_else(|| BackendError::Validation("range.from invalid format".to_string()))?;
        let to_day = range
            .to
            .get(0..10)
            .ok_or_else(|| BackendError::Validation("range.to invalid format".to_string()))?;
        let job_type_filter = super::normalize_optional_filter(job_type);
        let status_filter = super::normalize_status_filter(status)?;
        let job_type_param = job_type_filter.as_deref();
        let status_param = status_filter.as_deref();
        let total: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT v.day || '|' || v.job_type)
             FROM v_sync_daily_stats v
             WHERE v.day >= ?1
               AND v.day <= ?2
               AND (?3 IS NULL OR v.job_type = ?3)
               AND (
                    ?4 IS NULL
                    OR EXISTS(
                        SELECT 1
                        FROM job_runs jr
                        WHERE jr.job_type IN ('notion_smoke', 'notion_sync_once')
                          AND substr(jr.created_at, 1, 10) = v.day
                          AND jr.job_type = v.job_type
                          AND jr.status = ?4
                    )
               )",
            params![from_day, to_day, job_type_param, status_param],
            |r| r.get(0),
        )?;
        let page_size = page.page_size as i64;
        let offset = ((page.page - 1) * page.page_size) as i64;
        let mut stmt = conn.prepare(
            "SELECT
                day,
                job_type,
                run_count,
                success_runs,
                partial_runs,
                failed_runs,
                success_rate,
                avg_success_count,
                avg_fail_count
             FROM v_sync_daily_stats v
             WHERE v.day >= ?1
               AND v.day <= ?2
               AND (?3 IS NULL OR v.job_type = ?3)
               AND (
                    ?4 IS NULL
                    OR EXISTS(
                        SELECT 1
                        FROM job_runs jr
                        WHERE jr.job_type IN ('notion_smoke', 'notion_sync_once')
                          AND substr(jr.created_at, 1, 10) = v.day
                          AND jr.job_type = v.job_type
                          AND jr.status = ?4
                    )
               )
             ORDER BY v.day DESC, v.job_type ASC
             LIMIT ?5 OFFSET ?6",
        )?;
        let mapped = stmt.query_map(
            params![
                from_day,
                to_day,
                job_type_param,
                status_param,
                page_size,
                offset
            ],
            |row| {
                Ok(SyncDailyStat {
                    day: row.get(0)?,
                    job_type: row.get(1)?,
                    run_count: row.get::<_, i64>(2)? as u32,
                    success_runs: row.get::<_, i64>(3)? as u32,
                    partial_runs: row.get::<_, i64>(4)? as u32,
                    failed_runs: row.get::<_, i64>(5)? as u32,
                    success_rate: row.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
                    avg_success_count: row.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
                    avg_fail_count: row.get::<_, Option<f64>>(8)?.unwrap_or(0.0),
                })
            },
        )?;
        let mut items = Vec::new();
        for item in mapped {
            items.push(item?);
        }
        Ok(Paged {
            items,
            page: page.page,
            page_size: page.page_size,
            total: total.max(0) as u64,
        })
    }
}
