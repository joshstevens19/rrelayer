use std::time::SystemTimeError;

use alloy::transports::{RpcError, TransportErrorKind};
use thiserror::Error;

use super::{
    SendTransactionGasPriceError, TransactionQueueSendTransactionError, TransactionSentWithRelayer,
};
use crate::shared::{bad_request, internal_server_error, not_found, HttpError};
use crate::transaction::types::TransactionConversionError;
use crate::{
    postgres::PostgresError,
    relayer::RelayerId,
    transaction::types::{Transaction, TransactionId, TransactionStatus},
    WalletError,
};

#[derive(Error, Debug)]
pub enum ReplaceTransactionError {
    #[error("Send transaction error: {0}")]
    SendTransactionError(#[from] TransactionQueueSendTransactionError),

    #[error("Transaction could not be found: {0}")]
    TransactionNotFound(TransactionId),

    #[error("Could not read allowlists from db: {0}")]
    CouldNotReadAllowlistsFromDb(PostgresError),

    #[error("Relayer {0} is paused")]
    RelayerIsPaused(RelayerId),

    #[error("Relayer could not update the transaction in the db {0}")]
    CouldNotUpdateTransactionInDb(#[from] PostgresError),
}

impl From<ReplaceTransactionError> for HttpError {
    fn from(value: ReplaceTransactionError) -> Self {
        if matches!(value, ReplaceTransactionError::TransactionNotFound(_)) {
            return bad_request(value.to_string());
        }

        if matches!(value, ReplaceTransactionError::RelayerIsPaused(_)) {
            return bad_request(value.to_string());
        }

        internal_server_error(Some(value.to_string()))
    }
}

#[derive(Error, Debug)]
pub enum AddTransactionError {
    #[error("Transaction could not be saved in DB: {0}")]
    CouldNotSaveTransactionDb(PostgresError),

    #[error("Relayer could not be found: {0}")]
    RelayerNotFound(RelayerId),

    #[error("Could not read allowlists from db: {0}")]
    CouldNotReadAllowlistsFromDb(PostgresError),

    #[error("Relayer {0} is paused")]
    RelayerIsPaused(RelayerId),

    #[error("{0}")]
    TransactionGasPriceError(#[from] SendTransactionGasPriceError),

    #[error("{0}")]
    ComputeTransactionHashError(#[from] WalletError),

    #[error("could not estimate gas limit - {0}")]
    TransactionEstimateGasError(RelayerId, RpcError<TransportErrorKind>),

    #[error("Could not get current on chain nonce for relayer {0} - {1}")]
    CouldNotGetCurrentOnChainNonce(RelayerId, RpcError<TransportErrorKind>),

    #[error("Conversion error: {0}")]
    TransactionConversionError(#[from] TransactionConversionError),

    #[error("Unsupported transaction type: {message}")]
    UnsupportedTransactionType { message: String },
}

impl From<AddTransactionError> for HttpError {
    fn from(value: AddTransactionError) -> Self {
        if matches!(value, AddTransactionError::RelayerIsPaused(_)) {
            return bad_request(value.to_string());
        }

        if matches!(value, AddTransactionError::RelayerNotFound(_)) {
            return not_found(value.to_string());
        }

        if matches!(value, AddTransactionError::UnsupportedTransactionType { .. }) {
            return bad_request(value.to_string());
        }

        internal_server_error(Some(value.to_string()))
    }
}

#[derive(Error, Debug)]
pub enum CancelTransactionError {
    #[error("Send transaction error: {0}")]
    SendTransactionError(#[from] TransactionQueueSendTransactionError),

    #[error("Could not update transaction in database: {0}")]
    CouldNotUpdateTransactionDb(PostgresError),

    #[error("Relayer could not be found: {0}")]
    RelayerNotFound(RelayerId),

    #[error("Relayer {0} is paused")]
    RelayerIsPaused(RelayerId),
}

impl From<CancelTransactionError> for HttpError {
    fn from(value: CancelTransactionError) -> Self {
        if matches!(value, CancelTransactionError::RelayerIsPaused(_)) {
            return bad_request(value.to_string());
        }

        if matches!(value, CancelTransactionError::RelayerNotFound(_)) {
            return not_found(value.to_string());
        }

        internal_server_error(Some(value.to_string()))
    }
}

#[derive(Error, Debug)]
pub enum ProcessPendingTransactionError {
    #[error("Relayer transactions queue not found for relayer {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error("Send transaction error: {0}")]
    SendTransactionError(#[from] TransactionQueueSendTransactionError),

    #[error("Transaction could not be sent due to gas calculation error for relayer {0}: tx {1}")]
    GasCalculationError(RelayerId, Transaction),

    #[error("{0}")]
    MovePendingTransactionToInmempoolError(#[from] MovePendingTransactionToInmempoolError),

    #[error("Transaction estimate gas error: {0}")]
    TransactionEstimateGasError(#[from] RpcError<TransportErrorKind>),

    #[error("Transaction could not be updated in DB: {0}")]
    DbError(#[from] PostgresError),
}

#[derive(Error, Debug)]
pub enum ProcessInmempoolTransactionError {
    #[error("Relayer transactions queue not found for relayer {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error("Send transaction error: {0}")]
    SendTransactionError(#[from] TransactionQueueSendTransactionError),

    #[error(
        "Transaction status {3} could not be updated in the database for relayer {0}: tx {1} - error {2}"
    )]
    CouldNotUpdateTransactionStatusInTheDatabase(
        RelayerId,
        Transaction,
        TransactionStatus,
        PostgresError,
    ),

    #[error("{0}")]
    MoveInmempoolTransactionToMinedError(#[from] MoveInmempoolTransactionToMinedError),

    #[error("Could not read transaction receipt relayer {0} tx - {1} error - {2}")]
    CouldNotGetTransactionReceipt(RelayerId, Transaction, RpcError<TransportErrorKind>),

    #[error("Transaction does not have an hash for relayer - {1} tx - {0}")]
    UnknownTransactionHash(RelayerId, Transaction),
}

#[derive(Error, Debug)]
pub enum ProcessMinedTransactionError {
    #[error("Relayer transactions queue not found for relayer {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error(
        "Transaction confirmed not be saved to the database for relayer {0}: tx {1} - error {2}"
    )]
    TransactionConfirmedNotSaveToDatabase(RelayerId, Transaction, PostgresError),

    #[error("Relayer transaction has no mined at for relayer {0} - tx {1}")]
    NoMinedAt(RelayerId, Transaction),

    #[error("Relayer transaction has no mined at for relayer {0} - tx {1} - error {2}")]
    MinedAtTimeError(RelayerId, Transaction, SystemTimeError),

    #[error("Could not read transaction receipt relayer {0} tx - {1} error - {2}")]
    CouldNotGetTransactionReceipt(RelayerId, Transaction, RpcError<TransportErrorKind>),
}

#[derive(Error, Debug)]
pub enum MovePendingTransactionToInmempoolError {
    #[error("Relayer transaction not found for relayer {0} and tx {1}")]
    TransactionNotFound(RelayerId, TransactionSentWithRelayer),

    #[error("Relayer transaction ID does not match for relayer {0} - tx sent {1} - tx at front of queue {2}")]
    TransactionIdDoesNotMatch(RelayerId, TransactionSentWithRelayer, Transaction),
}

#[derive(Error, Debug)]
pub enum MoveInmempoolTransactionToMinedError {
    #[error("Relayer transaction not found for relayer {0} and tx {1}")]
    TransactionNotFound(RelayerId, TransactionId),

    #[error("Relayer transaction ID does not match for relayer {0} - tx sent {1} - tx at front of queue {2}")]
    TransactionIdDoesNotMatch(RelayerId, TransactionId, Transaction),
}
