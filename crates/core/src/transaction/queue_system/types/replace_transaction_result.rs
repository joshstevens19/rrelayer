use crate::transaction::types::{TransactionHash, TransactionId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplaceTransactionResult {
    pub success: bool,
    pub replace_transaction_id: Option<TransactionId>,
    pub replace_transaction_hash: Option<TransactionHash>,
}

impl ReplaceTransactionResult {
    pub fn success(
        replace_transaction_id: TransactionId,
        replace_transaction_hash: TransactionHash,
    ) -> Self {
        Self {
            success: true,
            replace_transaction_id: Some(replace_transaction_id),
            replace_transaction_hash: Some(replace_transaction_hash),
        }
    }

    pub fn failed() -> Self {
        Self { success: false, replace_transaction_id: None, replace_transaction_hash: None }
    }
}
