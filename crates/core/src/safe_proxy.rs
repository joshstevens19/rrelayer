use alloy::primitives::{keccak256, Address, Bytes, U256};
use alloy::providers::Provider;
use alloy::rpc::types::serde_helpers::WithOtherFields;
use alloy::sol;
use alloy::sol_types::{SolCall, SolStruct};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::network::ChainId;
use crate::{
    provider::EvmProvider,
    shared::common_types::EvmAddress,
    transaction::types::{TransactionData, TransactionValue},
    SafeProxyConfig,
};

#[derive(Error, Debug)]
pub enum SafeProxyError {
    #[error("Relayer {0} is not authorized for safe proxy {1}")]
    RelayerNotAuthorized(EvmAddress, EvmAddress),

    #[error("Safe proxy not found for relayer {0}")]
    SafeProxyNotFound(EvmAddress),

    #[error("Invalid transaction data for safe proxy")]
    InvalidTransactionData,

    #[error("Safe contract call encoding failed: {0}")]
    EncodingFailed(String),

    #[error("Safe contract interaction failed: {0}")]
    SafeContractError(String),

    #[error("Safe signature creation failed: {0}")]
    SignatureError(String),
}

sol! {
    #[sol(rpc)]
    interface ISafeContract {
        function nonce() external view returns (uint256);
        function getTransactionHash(
            address to,
            uint256 value,
            bytes calldata data,
            uint8 operation,
            uint256 safeTxGas,
            uint256 baseGas,
            uint256 gasPrice,
            address gasToken,
            address refundReceiver,
            uint256 _nonce
        ) external view returns (bytes32);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeTransaction {
    pub to: EvmAddress,
    pub value: U256,
    pub data: Bytes,
    pub operation: u8, // 0 for CALL, 1 for DELEGATECALL
    pub safe_tx_gas: U256,
    pub base_gas: U256,
    pub gas_price: U256,
    pub gas_token: EvmAddress,       // Address::ZERO for ETH
    pub refund_receiver: EvmAddress, // Address::ZERO for tx.origin
    pub nonce: U256,
}

impl SafeTransaction {
    pub fn new(to: EvmAddress, value: U256, data: Bytes, nonce: U256) -> Self {
        Self {
            to,
            value,
            data,
            operation: 0, // CALL operation
            safe_tx_gas: U256::ZERO,
            base_gas: U256::ZERO,
            gas_price: U256::ZERO,
            gas_token: EvmAddress::new(Address::ZERO),
            refund_receiver: EvmAddress::new(Address::ZERO),
            nonce,
        }
    }
}

#[derive(Clone)]
pub struct SafeProxyManager {
    configs: Vec<SafeProxyConfig>,
}

impl SafeProxyManager {
    pub fn new(configs: Vec<SafeProxyConfig>) -> Self {
        Self { configs }
    }

    pub fn get_safe_proxy_for_relayer(
        &self,
        relayer_address: &EvmAddress,
        chain_id: ChainId,
    ) -> Option<EvmAddress> {
        self.configs
            .iter()
            .find(|config| config.relayers.contains(relayer_address) && config.chain_id == chain_id)
            .map(|config| config.address)
    }

    pub fn is_relayer_authorized(
        &self,
        relayer_address: &EvmAddress,
        safe_address: &EvmAddress,
    ) -> bool {
        self.configs.iter().any(|config| {
            config.address == *safe_address && config.relayers.contains(relayer_address)
        })
    }

    /// Convert a regular transaction to a safe transaction
    pub fn wrap_transaction_for_safe(
        &self,
        relayer_address: &EvmAddress,
        chain_id: ChainId,
        original_to: EvmAddress,
        original_value: TransactionValue,
        original_data: TransactionData,
        safe_nonce: U256,
    ) -> Result<(EvmAddress, SafeTransaction), SafeProxyError> {
        let safe_address = self
            .get_safe_proxy_for_relayer(relayer_address, chain_id)
            .ok_or_else(|| SafeProxyError::SafeProxyNotFound(*relayer_address))?;

        let value = original_value.into_inner();
        let data = original_data.into_inner();

        let safe_tx = SafeTransaction::new(original_to, value, data, safe_nonce);

        Ok((safe_address, safe_tx))
    }

    /// Encode a safe transaction for execution using proper ABI encoding
    /// This creates the execTransaction call data for the Safe contract
    pub fn encode_safe_transaction(
        &self,
        safe_tx: &SafeTransaction,
        signatures: Bytes,
    ) -> Result<Bytes, SafeProxyError> {
        sol! {
            interface ISafe {
                function execTransaction(
                    address to,
                    uint256 value,
                    bytes calldata data,
                    uint8 operation,
                    uint256 safeTxGas,
                    uint256 baseGas,
                    uint256 gasPrice,
                    address gasToken,
                    address refundReceiver,
                    bytes memory signatures
                ) external payable returns (bool success);
            }
        }

        let call = ISafe::execTransactionCall {
            to: safe_tx.to.into_address(),
            value: safe_tx.value,
            data: safe_tx.data.clone(),
            operation: safe_tx.operation,
            safeTxGas: safe_tx.safe_tx_gas,
            baseGas: safe_tx.base_gas,
            gasPrice: safe_tx.gas_price,
            gasToken: safe_tx.gas_token.into_address(),
            refundReceiver: safe_tx.refund_receiver.into_address(),
            signatures,
        };

        Ok(call.abi_encode().into())
    }

    /// Get the transaction hash for a safe transaction using proper EIP-712 signing
    pub fn get_safe_transaction_hash(
        &self,
        safe_address: &EvmAddress,
        safe_tx: &SafeTransaction,
        chain_id: u64,
    ) -> Result<[u8; 32], SafeProxyError> {
        sol! {
            #[derive(Debug)]
            struct SafeTx {
                address to;
                uint256 value;
                bytes data;
                uint8 operation;
                uint256 safeTxGas;
                uint256 baseGas;
                uint256 gasPrice;
                address gasToken;
                address refundReceiver;
                uint256 nonce;
            }
        }

        let safe_tx_struct = SafeTx {
            to: safe_tx.to.into_address(),
            value: safe_tx.value,
            data: safe_tx.data.clone(),
            operation: safe_tx.operation,
            safeTxGas: safe_tx.safe_tx_gas,
            baseGas: safe_tx.base_gas,
            gasPrice: safe_tx.gas_price,
            gasToken: safe_tx.gas_token.into_address(),
            refundReceiver: safe_tx.refund_receiver.into_address(),
            nonce: safe_tx.nonce,
        };

        let safe_tx_hash = safe_tx_struct.eip712_hash_struct();
        let domain_separator = self.get_domain_separator(safe_address, chain_id)?;

        let mut final_data = Vec::new();
        final_data.push(0x19);
        final_data.push(0x01);
        final_data.extend_from_slice(&domain_separator);
        final_data.extend_from_slice(safe_tx_hash.as_slice());

        let final_hash = keccak256(&final_data);
        Ok(final_hash.into())
    }

    fn get_domain_separator(
        &self,
        safe_address: &EvmAddress,
        chain_id: u64,
    ) -> Result<[u8; 32], SafeProxyError> {
        sol! {
            #[derive(Debug)]
            struct EIP712Domain {
                uint256 chainId;
                address verifyingContract;
            }
        }

        let domain = EIP712Domain {
            chainId: U256::from(chain_id),
            verifyingContract: safe_address.into_address(),
        };

        Ok(domain.eip712_hash_struct().into())
    }

    /// Fetches the current nonce from a Safe contract.
    pub async fn get_safe_nonce(
        &self,
        provider: &EvmProvider,
        safe_address: &EvmAddress,
    ) -> Result<U256, SafeProxyError> {
        let nonce_call = ISafeContract::nonceCall {};

        let call_tx = WithOtherFields::new(alloy::rpc::types::TransactionRequest {
            to: Some(alloy::primitives::TxKind::Call((*safe_address).into())),
            input: Some(nonce_call.abi_encode().into()).into(),
            ..Default::default()
        });

        match provider.rpc_client().call(&call_tx).await {
            Ok(result) => match ISafeContract::nonceCall::abi_decode_returns(&result, false) {
                Ok(nonce) => Ok(nonce._0),
                Err(e) => Err(SafeProxyError::SafeContractError(format!(
                    "Failed to decode Safe nonce response: {}",
                    e
                ))),
            },
            Err(e) => Err(SafeProxyError::SafeContractError(format!(
                "Failed to call nonce on Safe contract: {}",
                e
            ))),
        }
    }

    /// Creates a signature for a Safe transaction.
    pub async fn create_safe_signature(
        &self,
        provider: &EvmProvider,
        wallet_index: u32,
        safe_address: &EvmAddress,
        safe_tx: &SafeTransaction,
        chain_id: u64,
    ) -> Result<Bytes, SafeProxyError> {
        let tx_hash = self.get_safe_transaction_hash(safe_address, safe_tx, chain_id)?;

        let hash_hex = format!("0x{}", hex::encode(tx_hash));

        // Sign the hash as text using the provider's text signing capability
        match provider.sign_text(&wallet_index, &hash_hex).await {
            Ok(signature) => {
                // Convert signature to Safe format: r + s + v
                // Safe expects v = recovery_id + 4 for ECDSA signatures
                let mut sig_bytes = Vec::new();
                sig_bytes.extend_from_slice(&signature.r().to_be_bytes::<32>());
                sig_bytes.extend_from_slice(&signature.s().to_be_bytes::<32>());

                // Safe requires v = recovery_id + 4 for ECDSA signatures
                let recovery_id = if signature.v() { 1u8 } else { 0u8 };
                sig_bytes.push(recovery_id + 4);

                Ok(sig_bytes.into())
            }
            Err(e) => Err(SafeProxyError::SignatureError(format!(
                "Failed to sign Safe transaction: {}",
                e
            ))),
        }
    }

    /// Creates a Safe transaction with proper nonce and signature, ready for execution.
    pub async fn create_safe_transaction_with_signature(
        &self,
        provider: &EvmProvider,
        wallet_index: u32,
        safe_address: &EvmAddress,
        to: EvmAddress,
        value: U256,
        data: Bytes,
    ) -> Result<(SafeTransaction, Bytes), SafeProxyError> {
        let safe_nonce = self.get_safe_nonce(provider, safe_address).await?;

        let safe_tx = SafeTransaction::new(to, value, data, safe_nonce);

        let signatures = self
            .create_safe_signature(
                provider,
                wallet_index,
                safe_address,
                &safe_tx,
                provider.chain_id.u64(),
            )
            .await?;

        let encoded_data = self.encode_safe_transaction(&safe_tx, signatures)?;

        Ok((safe_tx, encoded_data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::common_types::EvmAddress;
    use alloy::primitives::address;

    #[test]
    fn test_safe_proxy_manager() {
        let relayer1 = EvmAddress::new(address!("26988BA8250E009DCC5DF543D78E2277E2AA900B"));
        let relayer2 = EvmAddress::new(address!("36988BA8250E009DCC5DF543D78E2277E2AA900B"));
        let safe_address = EvmAddress::new(address!("46988BA8250E009DCC5DF543D78E2277E2AA900B"));

        let config = SafeProxyConfig {
            address: safe_address,
            relayers: vec![relayer1, relayer2],
            chain_id: ChainId(1),
        };

        let manager = SafeProxyManager::new(vec![config]);

        assert_eq!(manager.get_safe_proxy_for_relayer(&relayer1, ChainId(1)), Some(safe_address));
        assert_eq!(manager.get_safe_proxy_for_relayer(&relayer2, ChainId(1)), Some(safe_address));
        assert_eq!(
            manager.get_safe_proxy_for_relayer(
                &EvmAddress::new(address!("56988BA8250E009DCC5DF543D78E2277E2AA900B")),
                ChainId(1)
            ),
            None
        );

        // Test authorization check
        assert!(manager.is_relayer_authorized(&relayer1, &safe_address));
        assert!(manager.is_relayer_authorized(&relayer2, &safe_address));
        assert!(!manager.is_relayer_authorized(
            &EvmAddress::new(address!("56988BA8250E009DCC5DF543D78E2277E2AA900B")),
            &safe_address
        ));
    }

    #[test]
    fn test_safe_transaction_creation() {
        let to = EvmAddress::new(address!("1234567890123456789012345678901234567890"));
        let value = U256::from(1000000000000000000u64);
        let data = Bytes::from(vec![0x12, 0x34, 0x56]);
        let nonce = U256::from(42);

        let safe_tx = SafeTransaction::new(to, value, data.clone(), nonce);

        assert_eq!(safe_tx.to, to);
        assert_eq!(safe_tx.value, value);
        assert_eq!(safe_tx.data, data);
        assert_eq!(safe_tx.nonce, nonce);
        assert_eq!(safe_tx.operation, 0); // CALL
    }
}
