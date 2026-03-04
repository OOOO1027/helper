use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PipelineState {
    Queued,
    Processing,
    Succeeded,
    RetryWait,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PipelineEvent {
    Start,
    Complete,
    TemporaryFailure,
    PermanentFailure,
    RetryReady,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedQueueItem {
    pub item_id: String,
    pub stage: String,
    pub error_code: String,
    pub error_message: String,
    pub attempt: u32,
}

pub fn transition(state: PipelineState, event: PipelineEvent) -> PipelineState {
    match (state, event) {
        (PipelineState::Queued, PipelineEvent::Start) => PipelineState::Processing,
        (PipelineState::Processing, PipelineEvent::Complete) => PipelineState::Succeeded,
        (PipelineState::Processing, PipelineEvent::TemporaryFailure) => PipelineState::RetryWait,
        (PipelineState::Processing, PipelineEvent::PermanentFailure) => PipelineState::Failed,
        (PipelineState::RetryWait, PipelineEvent::RetryReady) => PipelineState::Queued,
        (s, _) => s,
    }
}

pub fn next_retry_delay_ms(attempt: u32) -> Option<u64> {
    // 重试最多3次，策略 2^n + jitter
    if attempt >= 3 {
        return None;
    }
    let n = attempt + 1;
    let jitter = (attempt as u64 * 137) % 250;
    Some(((1u64 << n) * 1000) + jitter)
}

pub fn should_route_failed_queue(attempt: u32, temporary: bool) -> bool {
    if !temporary {
        return true;
    }
    next_retry_delay_ms(attempt).is_none()
}
