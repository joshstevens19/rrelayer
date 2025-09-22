use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use crate::yaml::TurnkeySigningKey;
use alloy::consensus::{TxEnvelope, TypedTransaction};
use alloy::dyn_abi::TypedData;
use alloy::primitives::{keccak256, PrimitiveSignature};
use alloy_rlp::Decodable;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyAccount {
    pub address: String,
    #[serde(rename = "walletId")]
    pub wallet_id: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
    pub path: String,
    pub curve: String,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyCreateAccountRequest {
    #[serde(rename = "type")]
    pub activity_type: String,
    #[serde(rename = "organizationId")]
    pub organization_id: String,
    pub parameters: TurnkeyCreateAccountParameters,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyCreateAccountParameters {
    #[serde(rename = "walletId")]
    pub wallet_id: String,
    pub curve: String,
    #[serde(rename = "pathFormat")]
    pub path_format: String,
    #[serde(rename = "pathIndex")]
    pub path_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyCreateAccountResponse {
    pub activity: TurnkeyActivity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyActivity {
    pub id: String,
    pub status: String,
    pub result: Option<TurnkeyActivityResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyActivityResult {
    #[serde(rename = "createWalletAccountsResult")]
    pub create_wallet_accounts_result: Option<TurnkeyCreateAccountResult>,
    #[serde(rename = "signTransactionResult")]
    pub sign_transaction_result: Option<TurnkeySignTransactionResult>,
    #[serde(rename = "signRawPayloadResult")]
    pub sign_raw_payload_result: Option<TurnkeySignRawPayloadResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyCreateAccountResult {
    pub addresses: Vec<TurnkeyAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeySignTransactionResult {
    #[serde(rename = "signedTransaction")]
    pub signed_transaction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeySignRawPayloadResult {
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeySignTransactionRequest {
    #[serde(rename = "type")]
    pub activity_type: String,
    #[serde(rename = "organizationId")]
    pub organization_id: String,
    pub parameters: TurnkeySignTransactionParameters,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeySignTransactionParameters {
    #[serde(rename = "unsignedTransaction")]
    pub unsigned_transaction: String,
    #[serde(rename = "walletAccountId")]
    pub wallet_account_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeySignRawPayloadRequest {
    #[serde(rename = "type")]
    pub activity_type: String,
    #[serde(rename = "organizationId")]
    pub organization_id: String,
    pub parameters: TurnkeySignRawPayloadParameters,
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeySignRawPayloadParameters {
    pub payload: String,
    pub encoding: String,
    #[serde(rename = "hashFunction")]
    pub hash_function: String,
    #[serde(rename = "walletAccountId")]
    pub wallet_account_id: String,
}

#[derive(Debug)]
pub struct TurnkeyWalletManager {
    pub api_public_key: String,
    pub api_private_key: String,
    pub organization_id: String,
    pub wallet_id: String,
    pub accounts: Mutex<HashMap<u32, TurnkeyAccount>>,
    pub client: reqwest::Client,
}

impl TurnkeyWalletManager {
    pub async fn new(config: TurnkeySigningKey) -> Result<Self, WalletError> {
        let client = reqwest::Client::new();
        let manager = Self {
            api_public_key: config.api_public_key,
            api_private_key: config.api_private_key,
            organization_id: config.organization_id,
            wallet_id: config.wallet_id,
            accounts: Mutex::new(HashMap::new()),
            client,
        };

        Ok(manager)
    }

    fn create_signature(&self, body: &str, timestamp: &str) -> Result<String, WalletError> {
        let message = format!("{}{}", body, timestamp);

        let private_key_bytes =
            general_purpose::STANDARD.decode(&self.api_private_key).map_err(|e| {
                WalletError::ApiError { message: format!("Failed to decode private key: {}", e) }
            })?;

        let mut mac = HmacSha256::new_from_slice(&private_key_bytes).map_err(|e| {
            WalletError::ApiError { message: format!("Failed to create HMAC: {}", e) }
        })?;

        mac.update(message.as_bytes());
        let signature = mac.finalize().into_bytes();

        Ok(general_purpose::STANDARD.encode(signature))
    }

    fn get_timestamp_ms() -> String {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis().to_string()
    }
}

#[async_trait]
impl WalletManagerTrait for TurnkeyWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        _chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        {
            let accounts = self.accounts.lock().await;
            if let Some(account) = accounts.get(&wallet_index) {
                return Ok(EvmAddress::from_str(&account.address).map_err(|e| {
                    WalletError::ApiError { message: format!("Invalid address format: {}", e) }
                })?);
            }
        }

        let timestamp = Self::get_timestamp_ms();
        let request = TurnkeyCreateAccountRequest {
            activity_type: "ACTIVITY_TYPE_CREATE_WALLET_ACCOUNTS".to_string(),
            organization_id: self.organization_id.clone(),
            parameters: TurnkeyCreateAccountParameters {
                wallet_id: self.wallet_id.clone(),
                curve: "CURVE_SECP256K1".to_string(),
                path_format: "PATH_FORMAT_BIP32".to_string(),
                path_index: wallet_index,
            },
            timestamp_ms: timestamp.clone(),
        };

        let body = serde_json::to_string(&request).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize request: {}", e),
        })?;

        let signature = self.create_signature(&body, &timestamp)?;

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/activities")
            .header("X-Turnkey-API-Public-Key", &self.api_public_key)
            .header("X-Turnkey-Timestamp", &timestamp)
            .header("X-Turnkey-Signature", &signature)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Failed to create wallet account: {}", error_text),
            });
        }

        let result: TurnkeyCreateAccountResponse = response.json().await?;

        if result.activity.status != "ACTIVITY_STATUS_COMPLETED" {
            return Err(WalletError::ApiError {
                message: format!("Wallet creation failed with status: {}", result.activity.status),
            });
        }

        let create_result =
            result.activity.result.and_then(|r| r.create_wallet_accounts_result).ok_or(
                WalletError::ApiError { message: "No create result in response".to_string() },
            )?;

        let account = create_result
            .addresses
            .into_iter()
            .next()
            .ok_or(WalletError::ApiError { message: "No account created".to_string() })?;

        let address = EvmAddress::from_str(&account.address).map_err(|e| {
            WalletError::ApiError { message: format!("Invalid address format: {}", e) }
        })?;

        {
            let mut accounts = self.accounts.lock().await;
            accounts.insert(wallet_index, account);
        }

        Ok(address)
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        self.create_wallet(wallet_index, chain_id).await
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        _chain_id: &ChainId,
    ) -> Result<PrimitiveSignature, WalletError> {
        let accounts = self.accounts.lock().await;
        let account = accounts
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;
        let account_id = account.account_id.clone();
        drop(accounts);

        let unsigned_transaction = serde_json::to_string(transaction).map_err(|e| {
            WalletError::ApiError { message: format!("Failed to serialize transaction: {}", e) }
        })?;

        let timestamp = Self::get_timestamp_ms();
        let request = TurnkeySignTransactionRequest {
            activity_type: "ACTIVITY_TYPE_SIGN_TRANSACTION_V2".to_string(),
            organization_id: self.organization_id.clone(),
            parameters: TurnkeySignTransactionParameters {
                unsigned_transaction,
                wallet_account_id: account_id,
            },
            timestamp_ms: timestamp.clone(),
        };

        let body = serde_json::to_string(&request).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize request: {}", e),
        })?;

        let signature = self.create_signature(&body, &timestamp)?;

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/activities")
            .header("X-Turnkey-API-Public-Key", &self.api_public_key)
            .header("X-Turnkey-Timestamp", &timestamp)
            .header("X-Turnkey-Signature", &signature)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Turnkey API error: {}", error_text),
            });
        }

        let result: TurnkeyCreateAccountResponse = response.json().await?;

        if result.activity.status != "ACTIVITY_STATUS_COMPLETED" {
            return Err(WalletError::ApiError {
                message: format!(
                    "Transaction signing failed with status: {}",
                    result.activity.status
                ),
            });
        }

        let sign_result =
            result.activity.result.and_then(|r| r.sign_transaction_result).ok_or(
                WalletError::ApiError { message: "No sign result in response".to_string() },
            )?;

        let signed_tx_hex = sign_result.signed_transaction.trim_start_matches("0x");
        let tx_bytes = hex::decode(signed_tx_hex)?;
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
        let accounts = self.accounts.lock().await;
        let account = accounts
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;
        let account_id = account.account_id.clone();
        drop(accounts);

        let message = format!("\x19Ethereum Signed Message:\n{}{}", text.len(), text);
        let message_hash = keccak256(message.as_bytes());

        let timestamp = Self::get_timestamp_ms();
        let request = TurnkeySignRawPayloadRequest {
            activity_type: "ACTIVITY_TYPE_SIGN_RAW_PAYLOAD".to_string(),
            organization_id: self.organization_id.clone(),
            parameters: TurnkeySignRawPayloadParameters {
                payload: format!("0x{}", hex::encode(message_hash)),
                encoding: "PAYLOAD_ENCODING_ETHEREUM_SIGNED_MESSAGE".to_string(),
                hash_function: "HASH_FUNCTION_KECCAK256".to_string(),
                wallet_account_id: account_id,
            },
            timestamp_ms: timestamp.clone(),
        };

        let body = serde_json::to_string(&request).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize request: {}", e),
        })?;

        let signature = self.create_signature(&body, &timestamp)?;

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/activities")
            .header("X-Turnkey-API-Public-Key", &self.api_public_key)
            .header("X-Turnkey-Timestamp", &timestamp)
            .header("X-Turnkey-Signature", &signature)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Turnkey API error: {}", error_text),
            });
        }

        let result: TurnkeyCreateAccountResponse = response.json().await?;

        if result.activity.status != "ACTIVITY_STATUS_COMPLETED" {
            return Err(WalletError::ApiError {
                message: format!("Text signing failed with status: {}", result.activity.status),
            });
        }

        let sign_result =
            result.activity.result.and_then(|r| r.sign_raw_payload_result).ok_or(
                WalletError::ApiError { message: "No sign result in response".to_string() },
            )?;

        let signature = PrimitiveSignature::from_str(&sign_result.signature)?;
        Ok(signature)
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<PrimitiveSignature, WalletError> {
        let accounts = self.accounts.lock().await;
        let account = accounts
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;
        let account_id = account.account_id.clone();
        drop(accounts);

        let encoded_data = typed_data.eip712_signing_hash().map_err(|e| WalletError::ApiError {
            message: format!("Failed to encode typed data: {}", e),
        })?;

        let timestamp = Self::get_timestamp_ms();
        let request = TurnkeySignRawPayloadRequest {
            activity_type: "ACTIVITY_TYPE_SIGN_RAW_PAYLOAD".to_string(),
            organization_id: self.organization_id.clone(),
            parameters: TurnkeySignRawPayloadParameters {
                payload: format!("0x{}", hex::encode(encoded_data)),
                encoding: "PAYLOAD_ENCODING_ETHEREUM_SIGNED_MESSAGE".to_string(),
                hash_function: "HASH_FUNCTION_KECCAK256".to_string(),
                wallet_account_id: account_id,
            },
            timestamp_ms: timestamp.clone(),
        };

        let body = serde_json::to_string(&request).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize request: {}", e),
        })?;

        let signature = self.create_signature(&body, &timestamp)?;

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/activities")
            .header("X-Turnkey-API-Public-Key", &self.api_public_key)
            .header("X-Turnkey-Timestamp", &timestamp)
            .header("X-Turnkey-Signature", &signature)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(WalletError::ApiError {
                message: format!("Turnkey API error: {}", error_text),
            });
        }

        let result: TurnkeyCreateAccountResponse = response.json().await?;

        if result.activity.status != "ACTIVITY_STATUS_COMPLETED" {
            return Err(WalletError::ApiError {
                message: format!(
                    "Typed data signing failed with status: {}",
                    result.activity.status
                ),
            });
        }

        let sign_result =
            result.activity.result.and_then(|r| r.sign_raw_payload_result).ok_or(
                WalletError::ApiError { message: "No sign result in response".to_string() },
            )?;

        let signature = PrimitiveSignature::from_str(&sign_result.signature)?;
        Ok(signature)
    }
}
