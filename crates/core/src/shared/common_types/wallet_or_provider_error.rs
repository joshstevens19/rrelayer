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
}
