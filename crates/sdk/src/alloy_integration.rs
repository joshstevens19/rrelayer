use alloy::{
    dyn_abi::TypedData,
    network::TransactionBuilder,
    primitives::{Address, B256, ChainId, Signature, TxKind, U256},
    rpc::types::TransactionRequest,
    signers::Signer,
};
use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;

use crate::{AdminRelayerClient, ApiSdkError, RelayerClient};
use rrelayer_core::{
    common_types::EvmAddress,
    relayer::RelayerId,
    transaction::api::RelayTransactionRequest,
    transaction::types::{TransactionData, TransactionValue},
};
use std::str::FromStr;

#[derive(Error, Debug)]
pub enum RelayerSignerError {
    #[error("Relayer API error: {0}")]
    ApiError(#[from] ApiSdkError),
    #[error("Invalid signature format")]
    InvalidSignature,
    #[error("Address conversion error")]
    AddressConversion,
}

/// A signer that routes all signing through your relayer service.
///
/// This is a drop-in replacement for Alloy signers like `PrivateKeySigner` that works
/// with existing Alloy code but routes all operations through your relayer infrastructure.
///
/// # Examples
///
/// ```rust,no_run
/// use alloy::signers::Signer;
/// use alloy::primitives::Address;
/// use alloy::dyn_abi::TypedData;
/// use rrelayer::{RelayerSigner, RelayerClient, RelayerClientConfig, RelayerClientAuth, RelayerId};
/// use std::{sync::Arc, str::FromStr};
///
/// # async fn example() -> anyhow::Result<()> {
/// // Create RelayerClient
/// let relayer_id = RelayerId::from_str("94afb207-bb47-4392-9229-ba87e4d783cb")?;
/// let config = RelayerClientConfig {
///     server_url: "http://localhost:8000".to_string(),
///     relayer_id: relayer_id.clone(),
///     auth: RelayerClientAuth::ApiKey { api_key: "your-key".to_string() },
///     fallback_speed: None,
/// };
/// let relayer_client = Arc::new(RelayerClient::new(config));
/// let address = Address::from_str("0x742d35cc6634c0532925a3b8d67e8000c942b1b5")?;
///
/// // Create from RelayerClient
/// let signer = RelayerSigner::from_relayer_client(
///     relayer_client,
///     address,
///     Some(1), // mainnet
/// );
///
/// // Sign messages (routes through relayer.sign().text())
/// let signature = signer.sign_message(b"hello").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RelayerSigner {
    client_type: RelayerClientType,
    address: Address,
    chain_id: Option<ChainId>,
}

#[derive(Debug, Clone)]
pub enum RelayerClientType {
    Relayer(Arc<RelayerClient>),
    Admin(Arc<AdminRelayerClient>),
}

impl RelayerSigner {
    /// Create a RelayerSigner from a RelayerClient.
    ///
    /// # Arguments
    ///
    /// * `client` - The RelayerClient to use for signing operations
    /// * `address` - The Ethereum address this signer represents
    /// * `chain_id` - Optional chain ID for EIP-155 replay protection
    pub fn from_relayer_client(
        client: Arc<RelayerClient>,
        address: Address,
        chain_id: Option<ChainId>,
    ) -> Self {
        Self { client_type: RelayerClientType::Relayer(client), address, chain_id }
    }

    /// Create a RelayerSigner from an AdminRelayerClient.
    ///
    /// # Arguments
    ///
    /// * `client` - The AdminRelayerClient to use for signing operations
    /// * `address` - The Ethereum address this signer represents  
    /// * `chain_id` - Optional chain ID for EIP-155 replay protection
    pub fn from_admin_client(
        client: Arc<AdminRelayerClient>,
        address: Address,
        chain_id: Option<ChainId>,
    ) -> Self {
        Self { client_type: RelayerClientType::Admin(client), address, chain_id }
    }

    /// Convenience constructor that automatically fetches the address from the relayer.
    ///
    /// # Arguments
    ///
    /// * `client` - The RelayerClient to use
    /// * `chain_id` - Optional chain ID for EIP-155 replay protection
    pub async fn from_relayer_client_auto_address(
        client: Arc<RelayerClient>,
        chain_id: Option<ChainId>,
    ) -> Result<Self, RelayerSignerError> {
        let relayer_address = client.address().await?;
        let address = relayer_address.into_address();

        Ok(Self::from_relayer_client(client, address, chain_id))
    }

    /// Convenience constructor that automatically fetches the address from the admin client.
    ///
    /// # Arguments
    ///
    /// * `client` - The AdminRelayerClient to use
    /// * `chain_id` - Optional chain ID for EIP-155 replay protection
    pub async fn from_admin_client_auto_address(
        client: Arc<AdminRelayerClient>,
        chain_id: Option<ChainId>,
    ) -> Result<Self, RelayerSignerError> {
        let relayer_address = client.address().await?;
        let address = relayer_address.into_address();

        Ok(Self::from_admin_client(client, address, chain_id))
    }

    /// Get the relayer ID associated with this signer.
    pub fn relayer_id(&self) -> &RelayerId {
        match &self.client_type {
            RelayerClientType::Relayer(client) => client.id(),
            RelayerClientType::Admin(client) => client.id(),
        }
    }

    /// Get access to the underlying client for advanced operations.
    pub fn client(&self) -> &RelayerClientType {
        &self.client_type
    }

    /// Get address of relayer
    pub fn address(&self) -> &Address {
        &self.address
    }
}

#[async_trait]
impl Signer for RelayerSigner {
    /// Get the signer's Ethereum address.
    fn address(&self) -> Address {
        self.address
    }

    /// Get the chain ID for EIP-155 replay protection.
    fn chain_id(&self) -> Option<ChainId> {
        self.chain_id
    }

    /// Set the chain ID for EIP-155 replay protection.
    fn set_chain_id(&mut self, chain_id: Option<ChainId>) {
        self.chain_id = chain_id;
    }

    /// Sign a hash using the relayer service.
    ///
    /// This routes the signing request through `relayer.sign().text()`.
    async fn sign_hash(&self, hash: &B256) -> alloy::signers::Result<Signature> {
        let hash_hex = format!("0x{}", hex::encode(hash));

        let sign_result = match &self.client_type {
            RelayerClientType::Relayer(client) => client.sign().text(&hash_hex, None).await,
            RelayerClientType::Admin(client) => client.sign().text(&hash_hex, None).await,
        }
        .map_err(alloy::signers::Error::other)?;

        Ok(sign_result.signature)
    }

    /// Sign dynamic typed data using the relayer service.
    ///
    /// This routes the signing request through `relayer.sign().typed_data()`.
    async fn sign_dynamic_typed_data(
        &self,
        payload: &TypedData,
    ) -> alloy::signers::Result<Signature> {
        let sign_result = match &self.client_type {
            RelayerClientType::Relayer(client) => client.sign().typed_data(payload, None).await,
            RelayerClientType::Admin(client) => client.sign().typed_data(payload, None).await,
        }
        .map_err(alloy::signers::Error::other)?;

        Ok(sign_result.signature)
    }
}

/// A provider wrapper that hijacks transaction sending to route through the relayer.
///
/// This allows you to use standard Alloy provider patterns while transparently routing
/// all transaction sending through your relayer infrastructure.
#[derive(Debug, Clone)]
pub struct RelayerProvider<P> {
    inner: P,
    relayer_signer: RelayerSigner,
}

impl<P> RelayerProvider<P> {
    /// Create a new RelayerProvider wrapping the given provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - The underlying provider to wrap
    /// * `relayer_signer` - The RelayerSigner to use for transaction operations
    pub fn new(provider: P, relayer_signer: RelayerSigner) -> Self {
        Self { inner: provider, relayer_signer }
    }

    /// Get access to the underlying provider.
    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// Get access to the relayer signer.
    pub fn relayer_signer(&self) -> &RelayerSigner {
        &self.relayer_signer
    }

    /// Send a transaction via the relayer service.
    ///
    /// This is a demonstration method showing how standard Alloy transaction
    /// patterns can be hijacked to route through the relayer.
    ///
    /// # Arguments
    ///
    /// * `to` - The recipient address
    /// * `value` - The value to send in wei
    ///
    /// # Returns
    ///
    /// The transaction ID from the relayer service
    /// Send a transaction using standard Alloy TransactionRequest.
    ///
    /// This accepts a standard Alloy `TransactionRequest` and automatically
    /// converts it to the relayer format, allowing seamless integration.
    ///
    /// # Arguments
    ///
    /// * `tx_request` - Standard Alloy TransactionRequest
    ///
    /// # Returns
    ///
    /// The transaction ID from the relayer service
    pub async fn send_transaction(
        &self,
        tx_request: &TransactionRequest,
    ) -> Result<String, RelayerSignerError> {
        println!("🔄 Sending transaction via relayer!");

        // Convert Alloy TransactionRequest to RelayTransactionRequest
        let relay_request = self.convert_transaction_request(tx_request)?;

        // Send through relayer
        let result = match &self.relayer_signer.client_type {
            RelayerClientType::Relayer(client) => {
                client.transaction().send(&relay_request, None).await
            }
            RelayerClientType::Admin(client) => {
                client.transaction().send(&relay_request, None).await
            }
        }?;

        Ok(result.id.to_string())
    }

    pub async fn send_transaction_via_relayer(
        &self,
        to: Address,
        value: U256,
    ) -> Result<String, RelayerSignerError> {
        // Convert to TransactionRequest and use the main method
        let tx_request = TransactionRequest::default().with_to(to).with_value(value);

        self.send_transaction(&tx_request).await
    }

    /// Convert Alloy TransactionRequest to RelayTransactionRequest
    fn convert_transaction_request(
        &self,
        tx_request: &TransactionRequest,
    ) -> Result<RelayTransactionRequest, RelayerSignerError> {
        // For now, use a simplified approach - we'll improve this as the API evolves

        // Extract to address - for now require it to be specified
        let to = match &tx_request.to {
            Some(TxKind::Call(to_addr)) => {
                let addr_str = format!("{:#x}", to_addr);
                EvmAddress::from_str(&addr_str)
                    .map_err(|_| RelayerSignerError::AddressConversion)?
            }
            Some(TxKind::Create) => {
                return Err(RelayerSignerError::InvalidSignature);
            }
            None => return Err(RelayerSignerError::InvalidSignature),
        };

        // Extract value (default to 0 if not specified)
        let value =
            tx_request.value.map(TransactionValue::new).unwrap_or_else(TransactionValue::zero);

        // Extract transaction data - simplified for now
        let data = match tx_request.input.input() {
            Some(input_bytes) if !input_bytes.is_empty() => {
                TransactionData::new(input_bytes.clone())
            }
            _ => TransactionData::empty(),
        };

        Ok(RelayTransactionRequest {
            to,
            value,
            data,
            speed: None, // Use relayer's default speed
            external_id: None,
            blobs: None, // EIP-4844 blobs not supported yet
        })
    }
}

/// Convenience function to wrap any provider with relayer functionality.
pub fn with_relayer<P>(provider: P, relayer_signer: RelayerSigner) -> RelayerProvider<P> {
    RelayerProvider::new(provider, relayer_signer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[test]
    fn test_relayer_signer_creation() {
        // This would require actual client setup, so we'll just test the structure
        let test_address = address!("742d35cc6634c0532925a3b8d67e8000c942b1b5");

        // Test that we can create the structure (compilation test)
        let _would_create = |client: Arc<RelayerClient>| {
            RelayerSigner::from_relayer_client(client, test_address, Some(1))
        };
    }

    #[test]
    fn test_provider_wrapper() {
        // Test that the provider wrapper compiles correctly
        let _test_fn = |signer: RelayerSigner| {
            let mock_provider = (); // In real use: HTTP provider
            with_relayer(mock_provider, signer)
        };
    }
}
