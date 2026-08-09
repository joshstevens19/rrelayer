use serde::{Deserialize, Serialize};

use crate::transaction::types::{TransactionHash, TransactionId};

/// Result of submitting a transaction at an explicit in-flight nonce.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendTransactionAtNonceResult {
    /// The transaction that was occupying the nonce.
    #[serde(rename = "replacedTransactionId")]
    pub replaced_transaction_id: TransactionId,

    /// The transaction now carrying the caller's payload at that nonce: a new
    /// same-nonce competitor when the holder was already broadcast, or the holder
    /// itself (edited in place) when it was still pending.
    #[serde(rename = "transactionId")]
    pub transaction_id: TransactionId,

    /// Broadcast hash when the replacement went straight to the mempool. `None` for
    /// in-place edits and cancels still pending broadcast - track the transaction id
    /// through the normal status reads instead.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hash: Option<TransactionHash>,
}
