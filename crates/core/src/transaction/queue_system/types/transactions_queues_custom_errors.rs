use std::time::SystemTimeError;

use alloy::transports::{RpcError, TransportErrorKind};
use thiserror::Error;

use super::{
    SendTransactionGasPriceError, TransactionQueueSendTransactionError, TransactionSentWithRelayer,
};
use crate::common_types::EvmAddress;
use crate::shared::{bad_request, forbidden, internal_server_error, not_found, HttpError};
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

    #[error("Nonce synchronization recovered, replacement transaction should be retried")]
    NonceSynchronizationRecovered,
}

impl From<ReplaceTransactionError> for HttpError {
    fn from(value: ReplaceTransactionError) -> Self {
        if matches!(value, ReplaceTransactionError::TransactionNotFound(_)) {
            return bad_request(value.to_string());
        }

        if matches!(value, ReplaceTransactionError::RelayerIsPaused(_)) {
            return forbidden(value.to_string());
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
            return forbidden(value.to_string());
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

    #[error("Nonce synchronization recovered, cancel transaction should be retried")]
    NonceSynchronizationRecovered,
}

impl From<CancelTransactionError> for HttpError {
    fn from(value: CancelTransactionError) -> Self {
        if matches!(value, CancelTransactionError::RelayerIsPaused(_)) {
            return forbidden(value.to_string());
        }

        if matches!(value, CancelTransactionError::RelayerNotFound(_)) {
            return not_found(value.to_string());
        }

        internal_server_error(Some(value.to_string()))
    }
}

#[derive(Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ProcessPendingTransactionError {
    #[error("Relayer transactions queue not found for relayer id {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error("Relayer id {0} / address {1} - Send transaction error: {2}")]
    SendTransactionError(RelayerId, EvmAddress, TransactionQueueSendTransactionError),

    #[error("Transaction could not be sent due to gas calculation error for relayer id {0} / address {1}: tx {2}")]
    GasCalculationError(RelayerId, EvmAddress, Transaction),

    #[error("Relayer id {0} / address {1} - {2}")]
    MovePendingTransactionToInmempoolError(
        RelayerId,
        EvmAddress,
        MovePendingTransactionToInmempoolError,
    ),

    #[error("Relayer id {0} / address {1} - Transaction estimate gas error: {2}")]
    TransactionEstimateGasError(RelayerId, EvmAddress, RpcError<TransportErrorKind>),

    #[error("Relayer id {0} / address {1} - Transaction could not be updated in DB: {2}")]
    DbError(RelayerId, EvmAddress, PostgresError),
}

#[derive(Error, Debug)]
pub enum ProcessInmempoolTransactionError {
    #[error("Relayer transactions queue not found for relayer {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error("Relayer id {0} / address {1} - Send transaction error: {2}")]
    SendTransactionError(RelayerId, EvmAddress, TransactionQueueSendTransactionError),

    #[error(
        "Transaction status {3} could not be updated in the database for relayer id {0} / address {1}: tx {2} - error {4}"
    )]
    CouldNotUpdateTransactionStatusInTheDatabase(
        RelayerId,
        EvmAddress,
        Transaction,
        TransactionStatus,
        PostgresError,
    ),

    #[error("Relayer id {0} / address {1} - {2}")]
    MoveInmempoolTransactionToMinedError(
        RelayerId,
        EvmAddress,
        MoveInmempoolTransactionToMinedError,
    ),

    #[error("Could not read transaction receipt relayer {0} tx - {1} error - {2}")]
    CouldNotGetTransactionReceipt(RelayerId, EvmAddress, Transaction, RpcError<TransportErrorKind>),

    #[error("Transaction does not have an hash for relayer id {0} / address {1} tx - {2}")]
    UnknownTransactionHash(RelayerId, EvmAddress, Transaction),
}

#[derive(Error, Debug)]
pub enum ProcessMinedTransactionError {
    #[error("Relayer transactions queue not found for relayer {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error(
        "Transaction confirmed not be saved to the database for  relayer id {0} / address {1}: tx {2} - error {3}"
    )]
    TransactionConfirmedNotSaveToDatabase(RelayerId, EvmAddress, Transaction, PostgresError),

    #[error("Relayer transaction has no mined at for relayer id {0} / address {1} - tx {2}")]
    NoMinedAt(RelayerId, EvmAddress, Transaction),

    #[error(
        "Relayer transaction has no mined at for relayer id {0} / address {1} - tx {2} - error {3}"
    )]
    MinedAtTimeError(RelayerId, EvmAddress, Transaction, SystemTimeError),

    #[error(
        "Could not read transaction receipt relayer id {0} / address {1} - tx - {2} error - {3}"
    )]
    CouldNotGetTransactionReceipt(RelayerId, EvmAddress, Transaction, RpcError<TransportErrorKind>),
}

#[derive(Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum MovePendingTransactionToInmempoolError {
    #[error("Relayer transaction not found for relayer id {0} / address {1} and tx {2}")]
    TransactionNotFound(RelayerId, EvmAddress, TransactionSentWithRelayer),

    #[error("Relayer transaction ID does not match for relayer id {0} / address {1} - tx sent {2} - tx at front of queue {3}")]
    TransactionIdDoesNotMatch(RelayerId, EvmAddress, TransactionSentWithRelayer, Transaction),
}

#[derive(Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum MoveInmempoolTransactionToMinedError {
    #[error("Relayer transaction not found for relayer id {0} / address {1} and tx {2}")]
    TransactionNotFound(RelayerId, EvmAddress, TransactionId),

    #[error("Relayer transaction ID does not match for relayer id {0} / address {1} - tx sent {2} - tx at front of queue {3}")]
    TransactionIdDoesNotMatch(RelayerId, EvmAddress, TransactionId, Transaction),
}

/// Result of moving a transaction from inmempool to mined with competition resolution details
#[derive(Debug, Clone)]
pub struct CompetitionResolutionResult {
    /// The transaction that won the race (was mined)
    pub winner: Transaction,
    /// The transaction status of the winner
    pub winner_status: TransactionStatus,
    /// The transaction that lost the race (if there was competition)
    pub loser: Option<Transaction>,
}
