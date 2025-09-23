use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use alloy::consensus::{TxEnvelope, TypedTransaction};
use alloy::dyn_abi::TypedData;
use alloy::primitives::PrimitiveSignature;
use alloy_rlp::Decodable;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivyWallet {
    pub id: String,
    pub address: EvmAddress,
    pub chain_type: String,
    pub created_at: u64,
}

#[derive(Debug)]
pub struct PrivyWalletManager {
    pub app_id: String,
    pub app_secret: String,
    pub wallets: Mutex<HashMap<u32, PrivyWallet>>,
    pub client: reqwest::Client,
}

impl PrivyWalletManager {
    pub async fn new(app_id: String, app_secret: String) -> Result<Self, WalletError> {
        let client = reqwest::Client::new();
        let manager = Self {
            app_id: app_id.clone(),
            app_secret,
            wallets: Mutex::new(HashMap::new()),
            client,
        };

        manager.load_wallets().await?;
        Ok(manager)
    }

    async fn load_wallets(&self) -> Result<(), WalletError> {
        let mut all_wallets = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let mut url = "https://api.privy.io/v1/wallets".to_string();
            if let Some(ref cursor_value) = cursor {
                url.push_str(&format!("?cursor={}", cursor_value));
            }

            let response = self
                .client
                .get(&url)
                .basic_auth(&self.app_id, Some(&self.app_secret))
                .header("privy-app-id", &self.app_id)
                .header("Content-Type", "application/json")
                .send()
                .await?;

            let response_data: serde_json::Value = response.json().await?;

            let wallets_in_page: Vec<PrivyWallet> =
                serde_json::from_value(response_data["data"].clone())?;

            all_wallets.extend(wallets_in_page);

            cursor = response_data["next_cursor"].as_str().map(|s| s.to_string());

            if cursor.is_none() {
                break;
            }
        }

        let mut wallets = self.wallets.lock().await;
        for (index, wallet) in all_wallets.into_iter().enumerate() {
            wallets.insert(index as u32, wallet);
        }

        Ok(())
    }
}

#[async_trait]
impl WalletManagerTrait for PrivyWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        _chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        self.load_wallets().await?;

        {
            let wallets = self.wallets.lock().await;
            if let Some(wallet) = wallets.get(&wallet_index) {
                return Ok(wallet.address);
            }
        }

        let response = self
            .client
            .post("https://api.privy.io/v1/wallets")
            .basic_auth(&self.app_id, Some(&self.app_secret))
            .header("privy-app-id", &self.app_id)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "chain_type": "ethereum"
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Failed to create wallet: {}", error_text),
            });
        }

        let new_wallet: PrivyWallet = response.json().await?;
        let address = new_wallet.address;

        {
            let mut wallets = self.wallets.lock().await;
            if !wallets.contains_key(&wallet_index) {
                wallets.insert(wallet_index, new_wallet);
            }
        }

        Ok(address)
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        _chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let wallets = self.wallets.lock().await;
        let wallet = wallets
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;
        Ok(wallet.address)
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        _chain_id: &ChainId,
    ) -> Result<PrimitiveSignature, WalletError> {
        let wallets = self.wallets.lock().await;
        let wallet = wallets
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;

        let privy_transaction = match transaction {
            TypedTransaction::Legacy(tx) => {
                let to_addr = match &tx.to {
                    alloy::primitives::TxKind::Call(addr) => Some(format!("0x{:x}", addr)),
                    alloy::primitives::TxKind::Create => None,
                };
                serde_json::json!({
                    "type": 0,
                    "to": to_addr,
                    "value": format!("0x{:x}", tx.value),
                    "data": format!("0x{}", hex::encode(&tx.input)),
                    "gas_limit": tx.gas_limit,
                    "gas_price": format!("0x{:x}", tx.gas_price),
                    "nonce": tx.nonce
                })
            },
            TypedTransaction::Eip2930(tx) => {
                let to_addr = match &tx.to {
                    alloy::primitives::TxKind::Call(addr) => Some(format!("0x{:x}", addr)),
                    alloy::primitives::TxKind::Create => None,
                };
                serde_json::json!({
                    "type": 1,
                    "to": to_addr,
                    "value": format!("0x{:x}", tx.value),
                    "data": format!("0x{}", hex::encode(&tx.input)),
                    "gas_limit": tx.gas_limit,
                    "gas_price": format!("0x{:x}", tx.gas_price),
                    "nonce": tx.nonce,
                    "chain_id": tx.chain_id
                })
            },
            TypedTransaction::Eip1559(tx) => {
                let to_addr = match &tx.to {
                    alloy::primitives::TxKind::Call(addr) => Some(format!("0x{:x}", addr)),
                    alloy::primitives::TxKind::Create => None,
                };
                serde_json::json!({
                    "type": 2,
                    "to": to_addr,
                    "value": format!("0x{:x}", tx.value),
                    "data": format!("0x{}", hex::encode(&tx.input)),
                    "gas_limit": tx.gas_limit,
                    "max_fee_per_gas": tx.max_fee_per_gas,
                    "max_priority_fee_per_gas": format!("0x{:x}", tx.max_priority_fee_per_gas),
                    "nonce": tx.nonce,
                    "chain_id": tx.chain_id
                })
            },
            TypedTransaction::Eip4844(_) => {
                return Err(WalletError::UnsupportedTransactionType {
                    tx_type: "EIP-4844 blob transactions are not supported by Privy wallet API".to_string(),
                })
            }
            _ => {
                return Err(WalletError::UnsupportedTransactionType {
                    tx_type: format!("Unsupported transaction type for Privy: {:?}", transaction),
                })
            }
        };

        let response = self
            .client
            .post(&format!("https://api.privy.io/v1/wallets/{}/rpc", wallet.id))
            .basic_auth(&self.app_id, Some(&self.app_secret))
            .header("privy-app-id", &self.app_id)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "method": "eth_signTransaction",
                "params": {
                    "transaction": privy_transaction
                }
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Privy API error: {}", error_text),
            });
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(error) = result.get("error") {
            return Err(WalletError::ApiError {
                message: format!("Privy signing error: {}", error),
            });
        }

        let signed_transaction_hex =
            result["data"]["signed_transaction"].as_str().ok_or(WalletError::ApiError {
                message: "No signed transaction in response data".to_string(),
            })?;

        let tx_bytes = hex::decode(signed_transaction_hex.trim_start_matches("0x"))?;
        let tx_envelope = TxEnvelope::decode(&mut tx_bytes.as_slice())?;

        let signature = match tx_envelope {
            TxEnvelope::Eip1559(signed_tx) => signed_tx.signature().clone(),
            TxEnvelope::Legacy(signed_tx) => signed_tx.signature().clone(),
            TxEnvelope::Eip2930(signed_tx) => signed_tx.signature().clone(),
            _ => {
                return Err(WalletError::UnsupportedTransactionType {
                    tx_type: "Unknown transaction envelope type".to_string(),
                })
            }
        };

        Ok(signature)
    }

    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
    ) -> Result<PrimitiveSignature, WalletError> {
        let wallets = self.wallets.lock().await;
        let wallet = wallets
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;

        let response = self
            .client
            .post(&format!("https://api.privy.io/v1/wallets/{}/rpc", wallet.id))
            .basic_auth(&self.app_id, Some(&self.app_secret))
            .header("privy-app-id", &self.app_id)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "method": "personal_sign",
                "params": {
                    "message": text,
                    "encoding": "utf-8"
                }
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Privy API error: {}", error_text),
            });
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(error) = result.get("error") {
            return Err(WalletError::ApiError {
                message: format!("Privy signing error: {}", error),
            });
        }

        let signature_hex = result["data"]["signature"].as_str().ok_or(WalletError::ApiError {
            message: "No signature in response data".to_string(),
        })?;

        let signature = PrimitiveSignature::from_str(signature_hex)?;
        Ok(signature)
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError> {
        let wallets = self.wallets.lock().await;
        let wallet = wallets
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;

        let privy_typed_data = serde_json::json!({
            "types": typed_data.resolver,
            "message": typed_data.message,
            "primary_type": typed_data.primary_type,
            "domain": typed_data.domain,
        });

        let response = self
            .client
            .post(&format!("https://api.privy.io/v1/wallets/{}/rpc", wallet.id))
            .basic_auth(&self.app_id, Some(&self.app_secret))
            .header("privy-app-id", &self.app_id)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "method": "eth_signTypedData_v4",
                "params": {
                    "typed_data": privy_typed_data
                }
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Privy API error: {}", error_text),
            });
        }

        let result: serde_json::Value = response.json().await?;

        if let Some(error) = result.get("error") {
            return Err(WalletError::ApiError {
                message: format!("Privy signing error: {}", error),
            });
        }

        let signature_hex = result["data"]["signature"].as_str().ok_or(WalletError::ApiError {
            message: "No signature in response data".to_string(),
        })?;

        let signature = PrimitiveSignature::from_str(signature_hex)?;
        Ok(signature)
    }

    fn supports_blobs(&self) -> bool {
        false
    }
}
