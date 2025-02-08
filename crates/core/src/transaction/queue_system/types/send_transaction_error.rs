use alloy::transports::{RpcError, TransportErrorKind};
use thiserror::Error;

use crate::provider::SendTransactionError;

#[derive(Error, Debug)]
pub enum SendTransactionGasPriceError {
    #[error("Gas calculation error")]
    GasCalculationError,

    #[error("Transaction has no last sent gas price object")]
    NoLastSentGas,
}

#[derive(Error, Debug)]
pub enum TransactionQueueSendTransactionError {
    #[error("Gas price too high")]
    GasPriceTooHigh,

    #[error("Gas calculation error")]
    GasCalculationError,

    #[error("Transaction send error: {0}")]
    TransactionSendError(SendTransactionError),

    #[error("Transaction could not be updated in DB: {0}")]
    CouldNotUpdateTransactionDb(tokio_postgres::Error),

    #[error("{0}")]
    SendTransactionGasPriceError(SendTransactionGasPriceError),

    #[error("Transaction estimate gas error: {0}")]
    TransactionEstimateGasError(RpcError<TransportErrorKind>),
}
