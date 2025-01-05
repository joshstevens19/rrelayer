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
