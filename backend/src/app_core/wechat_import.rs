use rusqlite::Connection;
use tracing::info;
use uuid::Uuid;

use crate::collectors::wechat_import::parse_wechat_files;
use crate::{BackendError, Result};

use super::{AppCore, ImportPersistSummary, ImportSummary, PipelinePreviewReq};

impl AppCore {
    pub fn import_wechat_files(&self, paths: &[String]) -> Result<ImportSummary> {
        if paths.is_empty() {
            return Err(BackendError::Validation(
                "paths must contain at least one file".to_string(),
            ));
        }
        let parsed = parse_wechat_files(paths);
        let total = paths.len() as u32;
        let imported = parsed.items.len() as u32;
        let duplicates = parsed.duplicates;
        let failed = parsed.errors.len() as u32;
        info!(total, imported, duplicates, failed, "wechat import summary");
        Ok(ImportSummary {
            batch_id: Uuid::new_v4().to_string(),
            total,
            imported,
            duplicates,
            failed,
        })
    }

    pub fn import_wechat_and_persist_with_conn(
        &self,
        conn: &mut Connection,
        paths: &[String],
    ) -> Result<ImportPersistSummary> {
        if paths.is_empty() {
            return Err(BackendError::Validation(
                "paths must contain at least one file".to_string(),
            ));
        }
        let parsed = parse_wechat_files(paths);
        let mut persisted = 0u32;
        let mut persist_failed = 0u32;

        for err in &parsed.errors {
            let _ = self.record_dead_letter_raw(
                conn,
                "wechat_import_parse",
                &err.path,
                &format!("{{\"path\":\"{}\"}}", super::escape_json(&err.path)),
                &err.code,
                &err.message,
            );
        }

        for item in &parsed.items {
            let req = PipelinePreviewReq {
                source: "wechat".to_string(),
                source_url: item.source_url.clone(),
                published_at: item.published_at.clone(),
                content_raw: item.content_raw.clone(),
            };
            match self.execute_pipeline_with_conn(conn, req) {
                Ok(_) => persisted += 1,
                Err(e) => {
                    persist_failed += 1;
                    if !matches!(e, BackendError::Storage(_) | BackendError::Internal(_)) {
                        let _ = self.record_dead_letter_raw(
                            conn,
                            "wechat_persist",
                            &item.path,
                            &format!("{{\"path\":\"{}\"}}", super::escape_json(&item.path)),
                            e.code().as_str(),
                            &e.to_string(),
                        );
                    }
                }
            }
        }

        Ok(ImportPersistSummary {
            batch_id: Uuid::new_v4().to_string(),
            total: paths.len() as u32,
            parsed_ok: parsed.items.len() as u32,
            duplicates: parsed.duplicates,
            parse_failed: parsed.errors.len() as u32,
            persisted,
            persist_failed,
        })
    }
}
