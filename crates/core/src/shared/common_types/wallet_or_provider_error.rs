use crate::shared::{internal_server_error, HttpError};
use alloy::{
    signers::local::LocalSignerError,
    transports::{RpcError, TransportErrorKind},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WalletOrProviderError {
    #[error("Wallet error: {0}")]
    WalletError(#[from] LocalSignerError),

    #[error("Provider error: {0}")]
    ProviderError(RpcError<TransportErrorKind>),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<WalletOrProviderError> for HttpError {
    fn from(value: WalletOrProviderError) -> Self {
        internal_server_error(Some(value.to_string()))
    }
}
