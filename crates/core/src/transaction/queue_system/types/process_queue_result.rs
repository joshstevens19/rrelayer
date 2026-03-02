pub trait ProcessResultSuccess {
    fn success_type() -> Self;
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessResult<T> {
    pub status: T,
    pub process_again_after: u64,
}

impl<T> ProcessResult<T> {
    pub fn other(status: T, process_again_after: Option<&u64>) -> Self {
        Self { status, process_again_after: *process_again_after.unwrap_or(&10) }
    }
}

impl<T: ProcessResultSuccess> ProcessResult<T> {
    pub fn success() -> Self {
        Self {
            status: T::success_type(),
            // wait a little but of time to not overload the queue processing
            process_again_after: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessPendingStatus {
    Success,
    RelayerPaused,
    NoPendingTransactions,
    GasPriceTooHigh,
    NonceSynchronized,
    SendErrorBackoff,
}

impl ProcessResultSuccess for ProcessPendingStatus {
    fn success_type() -> Self {
        ProcessPendingStatus::Success
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessInmempoolStatus {
    Success,
    StillInmempool,
    NoInmempoolTransactions,
    GasIncreased,
    NonceSynchronized,
}

impl ProcessResultSuccess for ProcessInmempoolStatus {
    fn success_type() -> Self {
        ProcessInmempoolStatus::Success
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessMinedStatus {
    Success,
    NotConfirmedYet,
    NoMinedTransactions,
}

impl ProcessResultSuccess for ProcessMinedStatus {
    fn success_type() -> Self {
        ProcessMinedStatus::Success
    }
}

/// Computes exponential backoff delay in milliseconds for send error retries.
/// Starts at 2 seconds and doubles each attempt, capped at 60 seconds.
pub fn compute_send_error_backoff_ms(attempt_count: u32) -> u64 {
    std::cmp::min(1000u64.saturating_mul(2u64.saturating_pow(attempt_count)), 60_000)
}

/// Maximum number of send attempts for noop transactions before marking as failed.
pub const MAX_NOOP_SEND_ATTEMPTS: u32 = 10;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff_calculation() {
        // Attempt 0: 1000 * 2^0 = 1000ms (1s)
        assert_eq!(compute_send_error_backoff_ms(0), 1_000);
        // Attempt 1: 1000 * 2^1 = 2000ms (2s)
        assert_eq!(compute_send_error_backoff_ms(1), 2_000);
        // Attempt 2: 1000 * 2^2 = 4000ms (4s)
        assert_eq!(compute_send_error_backoff_ms(2), 4_000);
        // Attempt 3: 1000 * 2^3 = 8000ms (8s)
        assert_eq!(compute_send_error_backoff_ms(3), 8_000);
        // Attempt 5: 1000 * 2^5 = 32000ms (32s)
        assert_eq!(compute_send_error_backoff_ms(5), 32_000);
        // Attempt 6: 1000 * 2^6 = 64000ms -> capped at 60000ms (60s)
        assert_eq!(compute_send_error_backoff_ms(6), 60_000);
        // Attempt 10: still capped at 60s
        assert_eq!(compute_send_error_backoff_ms(10), 60_000);
        // Very high attempt: no overflow, still capped
        assert_eq!(compute_send_error_backoff_ms(100), 60_000);
    }

    #[test]
    fn test_send_error_backoff_variant_exists() {
        let result = ProcessResult::<ProcessPendingStatus>::other(
            ProcessPendingStatus::SendErrorBackoff,
            Some(&5000),
        );
        assert_eq!(result.status, ProcessPendingStatus::SendErrorBackoff);
        assert_eq!(result.process_again_after, 5000);
    }

    #[test]
    fn test_process_result_default_delay() {
        let result = ProcessResult::<ProcessPendingStatus>::other(
            ProcessPendingStatus::SendErrorBackoff,
            None,
        );
        // Default should be 10ms when no delay specified
        assert_eq!(result.process_again_after, 10);
    }

    #[test]
    fn test_backoff_never_below_one_second() {
        for attempt in 0..100 {
            assert!(
                compute_send_error_backoff_ms(attempt) >= 1_000,
                "Backoff at attempt {} was {} which is below 1 second",
                attempt,
                compute_send_error_backoff_ms(attempt)
            );
        }
    }
}
