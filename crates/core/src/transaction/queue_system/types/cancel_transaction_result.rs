use crate::transaction::types::TransactionId;

#[derive(Debug, Clone)]
pub struct CancelTransactionResult {
    pub success: bool,
    pub cancel_transaction_id: Option<TransactionId>,
}

impl CancelTransactionResult {
    pub fn success(cancel_transaction_id: TransactionId) -> Self {
        Self { success: true, cancel_transaction_id: Some(cancel_transaction_id) }
    }

    pub fn failed() -> Self {
        Self { success: false, cancel_transaction_id: None }
    }
}
