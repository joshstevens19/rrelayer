use alloy::transports::{RpcError, TransportErrorKind};
use thiserror::Error;

use crate::{postgres::PostgresError, provider::SendTransactionError, SafeProxyError};

impl TransactionQueueSendTransactionError {
    /// Returns true if the underlying error is a transport/connection error
    /// (RPC unreachable, timeout, network down) as opposed to a server-returned
    /// error response. Transport errors should always be retried without counting
    /// toward failure limits.
    pub fn is_connection_error(&self) -> bool {
        match self {
            TransactionQueueSendTransactionError::TransactionEstimateGasError(rpc_error) => {
                rpc_error.is_transport_error()
            }
            TransactionQueueSendTransactionError::TransactionSendError(send_error) => {
                matches!(
                    send_error,
                    SendTransactionError::RpcError(rpc_error) if rpc_error.is_transport_error()
                )
            }
            _ => false,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::transports::TransportErrorKind;

    #[test]
    fn test_estimate_gas_transport_error_is_connection_error() {
        let err =
            TransactionQueueSendTransactionError::TransactionEstimateGasError(RpcError::Transport(
                TransportErrorKind::Custom("connection refused".to_string().into()),
            ));
        assert!(err.is_connection_error());
    }

    #[test]
    fn test_estimate_gas_error_resp_is_not_connection_error() {
        let err =
            TransactionQueueSendTransactionError::TransactionEstimateGasError(RpcError::NullResp);
        assert!(!err.is_connection_error());
    }

    #[test]
    fn test_send_error_rpc_transport_is_connection_error() {
        let err = TransactionQueueSendTransactionError::TransactionSendError(
            SendTransactionError::RpcError(RpcError::Transport(TransportErrorKind::Custom(
                "timeout".to_string().into(),
            ))),
        );
        assert!(err.is_connection_error());
    }

    #[test]
    fn test_send_error_rpc_non_transport_is_not_connection_error() {
        let err = TransactionQueueSendTransactionError::TransactionSendError(
            SendTransactionError::RpcError(RpcError::NullResp),
        );
        assert!(!err.is_connection_error());
    }

    #[test]
    fn test_send_error_internal_is_not_connection_error() {
        let err = TransactionQueueSendTransactionError::TransactionSendError(
            SendTransactionError::InternalError("some error".to_string()),
        );
        assert!(!err.is_connection_error());
    }

    #[test]
    fn test_gas_price_too_high_is_not_connection_error() {
        let err = TransactionQueueSendTransactionError::GasPriceTooHigh;
        assert!(!err.is_connection_error());
    }

    #[test]
    fn test_transaction_conversion_error_is_not_connection_error() {
        let err = TransactionQueueSendTransactionError::TransactionConversionError(
            "bad conversion".to_string(),
        );
        assert!(!err.is_connection_error());
    }
}
