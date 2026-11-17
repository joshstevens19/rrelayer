use alloy::transports::{RpcError, TransportErrorKind};
use thiserror::Error;

use crate::{postgres::PostgresError, provider::SendTransactionError, SafeProxyError};

#[derive(Error, Debug)]
pub enum SendTransactionGasPriceError {
    #[error("Gas calculation error")]
    GasCalculationError,

    #[error("Blob gas calculation error")]
    BlobGasCalculationError,

    #[error("Transaction has no last sent gas price object")]
    NoLastSentGas,
}

#[derive(Error, Debug)]
pub enum TransactionQueueSendTransactionError {
    #[error("Gas price too high")]
    GasPriceTooHigh,

    /// The transaction's per-transaction gas price ceiling blocked this bid: freeze
    /// semantics refused to go above the ceiling, or cap already has the ceiling bid
    /// live in the mempool. Nothing was broadcast - the last compliant bid (if any)
    /// stays live and the normal expiry machinery applies.
    #[error("Gas price ceiling reached")]
    GasPriceCeilingReached,

    #[error("Gas calculation error")]
    GasCalculationError,

    #[error("Transaction send error: {0}")]
    TransactionSendError(#[from] SendTransactionError),

    #[error("Transaction could not be updated in DB: {0}")]
    CouldNotUpdateTransactionDb(#[from] PostgresError),

    #[error("{0}")]
    SendTransactionGasPriceError(#[from] SendTransactionGasPriceError),

    #[error("Transaction estimate gas error: {0}")]
    TransactionEstimateGasError(RpcError<TransportErrorKind>),

    #[error("Transaction conversion error: {0}")]
    TransactionConversionError(String),

    #[error("Safe proxy error: {0}")]
    SafeProxyError(#[from] SafeProxyError),

    #[error("No transaction found in queue")]
    NoTransactionInQueue,
}
