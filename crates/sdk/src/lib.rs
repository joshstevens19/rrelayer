pub mod alloy_integration;
mod api;
mod clients;

pub use clients::{
    AdminRelayerClient, AdminRelayerClientAuth, AdminRelayerClientConfig, Client, CreateClientAuth,
    CreateClientConfig, CreateRelayerClientConfig, RelayerClient, RelayerClientAuth,
    RelayerClientConfig, TransactionCountType, create_client, create_relayer_client,
};

pub use api::types::{ApiResult, AuthConfig};
pub use api::{ApiSdkError, AuthenticationApi, NetworkApi, RelayerApi, SignApi, TransactionApi};

pub use alloy_integration::{RelayerClientType, RelayerProvider, RelayerSigner, with_relayer};

pub use alloy::dyn_abi::TypedData;
pub use alloy::network::AnyTransactionReceipt;
pub use alloy::primitives::Signature;

pub use rrelayer_core::{
    common_types::{EvmAddress, PagingContext, PagingResult},
    gas::GasEstimatorResult,
    network::Network,
    relayer::{CreateRelayerResult, GetRelayerResult, Relayer, RelayerId},
    signing::{SignTextResult, SignTypedDataResult, SignedTextHistory, SignedTypedDataHistory},
    transaction::{
        api::{
            CancelTransactionResponse, RelayTransactionRequest, RelayTransactionStatusResult,
            SendTransactionResult,
        },
        queue_system::ReplaceTransactionResult,
        types::{Transaction, TransactionData, TransactionId, TransactionSpeed, TransactionValue},
    },
};
