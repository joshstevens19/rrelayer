mod builders;
mod read;
mod write;

use crate::{
    gas::{BlobGasPriceResult, GasPriceResult},
    transaction::types::TransactionHash,
};

/// One durable broadcast candidate reconstructed from the transaction's live
/// row and append-only audit history. Transaction identity, nonce and relayer
/// remain on the parent transaction record.
#[derive(Clone, Debug)]
pub(crate) struct RecordedTransactionAttempt {
    pub(crate) hash: TransactionHash,
    pub(crate) sent_with_gas: Option<GasPriceResult>,
    pub(crate) sent_with_blob_gas: Option<BlobGasPriceResult>,
}
