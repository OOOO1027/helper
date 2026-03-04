pub mod ai_orchestrator;
pub mod app_core;
pub mod budget_guard;
pub mod collectors;
pub mod config;
pub mod env_runtime;
pub mod ipc;
pub mod pipeline;
pub mod storage;
pub mod sync_notion;

use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    IpcValidation,      // IPC-6001
    CollectUnsupported, // ING-1001
    StorageFailed,      // DB-4001
    InternalFailed,     // INT-9000
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::IpcValidation => "IPC-6001",
            ErrorCode::CollectUnsupported => "ING-1001",
            ErrorCode::StorageFailed => "DB-4001",
            ErrorCode::InternalFailed => "INT-9000",
        }
    }

    pub fn retryable(self) -> bool {
        matches!(self, ErrorCode::StorageFailed | ErrorCode::InternalFailed)
    }
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("storage failed: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("internal failed: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, BackendError>;

impl BackendError {
    pub fn code(&self) -> ErrorCode {
        match self {
            BackendError::Validation(_) => ErrorCode::IpcValidation,
            BackendError::NotFound(_) => ErrorCode::CollectUnsupported,
            BackendError::Storage(_) => ErrorCode::StorageFailed,
            BackendError::Internal(_) => ErrorCode::InternalFailed,
        }
    }

    pub fn retryable(&self) -> bool {
        self.code().retryable()
    }
}
