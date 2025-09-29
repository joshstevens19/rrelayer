mod api;
mod clients;

pub use clients::{
    AdminRelayerClient, AdminRelayerClientAuth, AdminRelayerClientConfig, Client, CreateClientAuth,
    CreateClientConfig, CreateRelayerClientConfig, RelayerClient, RelayerClientAuth,
    RelayerClientConfig, TransactionCountType, create_client, create_relayer_client,
};

pub use api::types::{ApiResult, AuthConfig};
pub use api::{ApiSdkError, AuthenticationApi, NetworkApi, RelayerApi, SignApi, TransactionApi};

pub use rrelayer_core::{
    common_types::{EvmAddress, PagingContext, PagingResult},
    gas::GasEstimatorResult,
    network::Network,
    relayer::{CreateRelayerResult, GetRelayerResult, Relayer, RelayerId},
    transaction::{
        api::{RelayTransactionRequest, SendTransactionResult, RelayTransactionStatusResult, CancelTransactionResponse},
        queue_system::ReplaceTransactionResult,
        types::{TransactionId, TransactionValue, TransactionData, TransactionSpeed, Transaction},
    },
    signing::{SignTextResult, SignedTextHistory, SignTypedDataResult, SignedTypedDataHistory}
};

pub use alloy::primitives::PrimitiveSignature;
pub use alloy::dyn_abi::TypedData;
pub use alloy::network::AnyTransactionReceipt;