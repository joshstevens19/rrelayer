use std::time::SystemTimeError;

use alloy::{
    signers::local::LocalSignerError,
    transports::{RpcError, TransportErrorKind},
};
use thiserror::Error;

use super::{
    SendTransactionGasPriceError, TransactionQueueSendTransactionError, TransactionSentWithRelayer,
};
use crate::{
    postgres::PostgresError,
    relayer::RelayerId,
    shared::common_types::EvmAddress,
    transaction::types::{Transaction, TransactionId, TransactionStatus},
};

#[derive(Error, Debug)]
pub enum ReplaceTransactionError {
    #[error("Send transaction error: {0}")]
    SendTransactionError(TransactionQueueSendTransactionError),

    #[error("Transaction could not be found: {0}")]
    TransactionNotFound(TransactionId),

    #[error("Could not read allowlists from db: {0}")]
    CouldNotReadAllowlistsFromDb(PostgresError),

    #[error("Relayer {0} could not send transactions to {1}")]
    RelayerNotAllowedToSendTransactionTo(RelayerId, EvmAddress),

    #[error("Relayer {0} is paused")]
    RelayerIsPaused(RelayerId),

    #[error("Relayer could not update the transaction in the db {0}")]
    CouldNotUpdateTransactionInDb(#[from] PostgresError),
}

#[derive(Error, Debug)]
pub enum AddTransactionError {
    #[error("Transaction could not be saved in DB: {0}")]
    CouldNotSaveTransactionDb(PostgresError),

    #[error("Relayer could not be found: {0}")]
    RelayerNotFound(RelayerId),

    #[error("Could not read allowlists from db: {0}")]
    CouldNotReadAllowlistsFromDb(PostgresError),

    #[error("Relayer {0} could not send transactions to {1}")]
    RelayerNotAllowedToSendTransactionTo(RelayerId, EvmAddress),

    #[error("Relayer {0} is paused")]
    RelayerIsPaused(RelayerId),

    #[error("{0}")]
    TransactionGasPriceError(SendTransactionGasPriceError),

    #[error("{0}")]
    ComputeTransactionHashError(LocalSignerError),

    #[error("could not estimate gas limit - {0}")]
    TransactionEstimateGasError(RelayerId, RpcError<TransportErrorKind>),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Could not get current on chain nonce for relayer {0} - {1}")]
    CouldNotGetCurrentOnChainNonce(RelayerId, RpcError<TransportErrorKind>),
}

#[derive(Error, Debug)]
pub enum CancelTransactionError {
    #[error("Send transaction error: {0}")]
    SendTransactionError(TransactionQueueSendTransactionError),

    #[error("Could not update transaction in database: {0}")]
    CouldNotUpdateTransactionDb(PostgresError),

    #[error("Relayer could not be found: {0}")]
    RelayerNotFound(RelayerId),

    #[error("Relayer {0} is paused")]
    RelayerIsPaused(RelayerId),
}

#[derive(Error, Debug)]
pub enum ProcessPendingTransactionError {
    #[error("Relayer transactions queue not found for relayer {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error("Send transaction error: {0}")]
    SendTransactionError(TransactionQueueSendTransactionError),

    #[error("Transaction could not be sent due to gas calculation error for relayer {0}: tx {1}")]
    GasCalculationError(RelayerId, Transaction),

    #[error("{0}")]
    MovePendingTransactionToInmempoolError(MovePendingTransactionToInmempoolError),

    #[error("Transaction estimate gas error: {0}")]
    TransactionEstimateGasError(RpcError<TransportErrorKind>),

    #[error("Transaction could not be updated in DB: {0}")]
    DbError(PostgresError),
}

#[derive(Error, Debug)]
pub enum ProcessInmempoolTransactionError {
    #[error("Relayer transactions queue not found for relayer {0}")]
    RelayerTransactionsQueueNotFound(RelayerId),

    #[error("Send transaction error: {0}")]
    SendTransactionError(TransactionQueueSendTransactionError),

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
    MoveInmempoolTransactionToMinedError(MoveInmempoolTransactionToMinedError),

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
