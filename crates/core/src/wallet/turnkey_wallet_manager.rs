use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use crate::yaml::TurnkeySigningProviderConfig;
use alloy::consensus::{TxEnvelope, TypedTransaction};
use alloy::dyn_abi::TypedData;
use alloy::primitives::{keccak256, Signature};
use alloy_rlp::Decodable;
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use p256::{
    ecdsa::{signature::Signer, Signature as EcdsaSignature, SigningKey},
    SecretKey,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyAccount {
    pub address: String,
    #[serde(rename = "walletId")]
    pub wallet_id: String,
    #[serde(rename = "walletAccountId")]
    pub account_id: String,
    pub path: String,
    pub curve: String,
    #[serde(rename = "pathFormat")]
    pub path_format: String,
    #[serde(rename = "addressFormat")]
    pub address_format: String,
    #[serde(rename = "publicKey")]
    pub public_key: String,
    #[serde(rename = "createdAt")]
    pub created_at: Option<serde_json::Value>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<serde_json::Value>,
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
pub struct TurnkeyCreateWalletAccountsResponse {
    pub activity: TurnkeyCreateWalletActivity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyCreateWalletActivity {
    pub id: String,
    pub status: String,
    pub result: Option<TurnkeyCreateWalletActivityResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyCreateWalletActivityResult {
    #[serde(rename = "createWalletAccountsResult")]
    pub create_wallet_accounts_result: Option<TurnkeyCreateWalletAccountsResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnkeyCreateWalletAccountsResult {
    pub addresses: Vec<String>,
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
    pub r: String,
    pub s: String,
    pub v: String,
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
    pub async fn new(config: TurnkeySigningProviderConfig) -> Result<Self, WalletError> {
        let client = reqwest::Client::new();
        let manager = Self {
            api_public_key: config.api_public_key,
            api_private_key: config.api_private_key,
            organization_id: config.organization_id,
            wallet_id: config.wallet_id,
            accounts: Mutex::new(HashMap::new()),
            client,
        };

        manager.load_accounts().await?;
        Ok(manager)
    }

    fn create_stamp(&self, body: &str) -> Result<String, WalletError> {
        let private_key_bytes = hex::decode(&self.api_private_key).map_err(|e| {
            error!("Failed to decode Turnkey private key: {}", e);
            WalletError::ApiError { message: format!("Failed to decode private key: {}", e) }
        })?;

        let secret_key = SecretKey::from_slice(&private_key_bytes).map_err(|e| {
            error!("Failed to create secret key from bytes: {}", e);
            WalletError::ApiError { message: format!("Failed to create secret key: {}", e) }
        })?;

        let signing_key = SigningKey::from(&secret_key);

        let signature: EcdsaSignature = signing_key.sign(body.as_bytes());
        let signature_bytes = signature.to_der();
        let signature_hex = hex::encode(&signature_bytes);

        let stamp_obj = serde_json::json!({
            "publicKey": self.api_public_key,
            "signature": signature_hex,
            "scheme": "SIGNATURE_SCHEME_TK_API_P256"
        });

        let stamp_json = serde_json::to_string(&stamp_obj).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize stamp: {}", e),
        })?;

        let encoded_stamp = general_purpose::URL_SAFE_NO_PAD.encode(stamp_json);

        Ok(encoded_stamp)
    }

    fn get_timestamp_ms() -> String {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis().to_string()
    }

    async fn load_accounts(&self) -> Result<(), WalletError> {
        let request = serde_json::json!({
            "organizationId": self.organization_id,
            "walletId": self.wallet_id
        });

        let body = serde_json::to_string(&request).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize request: {}", e),
        })?;

        let stamp = self.create_stamp(&body)?;

        info!("Loading existing Turnkey accounts for wallet {}", self.wallet_id);

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/query/list_wallet_accounts")
            .header("X-Stamp", &stamp)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            info!("No existing accounts found or error loading accounts: {}", error_text);
            return Ok(());
        }

        #[derive(Deserialize)]
        struct ListAccountsResponse {
            accounts: Vec<TurnkeyAccount>,
        }

        let result: ListAccountsResponse = response.json().await?;

        let mut accounts = self.accounts.lock().await;

        for account in result.accounts {
            if let Some(index) = self.extract_wallet_index_from_path(&account.path) {
                info!(
                    "Loaded existing Turnkey account: index {}, address {}, path {}",
                    index, account.address, account.path
                );
                accounts.insert(index, account);
            }
        }

        info!("Loaded {} existing Turnkey accounts", accounts.len());

        Ok(())
    }

    fn extract_wallet_index_from_path(&self, path: &str) -> Option<u32> {
        // Extract index from path like "m/44'/60'/0'/0/{index}"
        if let Some(last_part) = path.split('/').next_back() {
            last_part.parse::<u32>().ok()
        } else {
            None
        }
    }
}

#[async_trait]
impl WalletManagerTrait for TurnkeyWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        _chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        self.load_accounts().await?;

        // Check if account already exists in our loaded cache
        {
            let accounts = self.accounts.lock().await;
            if let Some(account) = accounts.get(&wallet_index) {
                return EvmAddress::from_str(&account.address).map_err(|e| {
                    WalletError::ApiError { message: format!("Invalid address format: {}", e) }
                });
            }
        }

        let timestamp = Self::get_timestamp_ms();
        let request = serde_json::json!({
            "type": "ACTIVITY_TYPE_CREATE_WALLET_ACCOUNTS",
            "timestampMs": timestamp,
            "organizationId": self.organization_id,
            "parameters": {
                "walletId": self.wallet_id,
                "accounts": [{
                    "curve": "CURVE_SECP256K1",
                    "pathFormat": "PATH_FORMAT_BIP32",
                    "path": format!("m/44'/60'/0'/0/{}", wallet_index),
                    "addressFormat": "ADDRESS_FORMAT_ETHEREUM"
                }]
            }
        });

        let body = serde_json::to_string(&request).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize request: {}", e),
        })?;

        let stamp = self.create_stamp(&body)?;

        info!(
            "Turnkey create_wallet request - URL: {}, body: {}, timestamp: {}, public_key: {}",
            "https://api.turnkey.com/public/v1/submit/create_wallet_accounts",
            body,
            timestamp,
            self.api_public_key
        );

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/submit/create_wallet_accounts")
            .header("X-Stamp", &stamp)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        info!(
            "Turnkey create_wallet response - status: {}, headers: {:?}",
            response.status(),
            response.headers()
        );

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("Turnkey create_wallet failed - status: {}, error: {}", status, error_text);
            return Err(WalletError::ApiError {
                message: format!("Failed to create wallet account: {}", error_text),
            });
        }

        let result: TurnkeyCreateWalletAccountsResponse = response.json().await?;

        if result.activity.status != "ACTIVITY_STATUS_COMPLETED" {
            return Err(WalletError::ApiError {
                message: format!("Wallet creation failed with status: {}", result.activity.status),
            });
        }

        let create_result =
            result.activity.result.and_then(|r| r.create_wallet_accounts_result).ok_or(
                WalletError::ApiError { message: "No create result in response".to_string() },
            )?;

        let address_str = create_result
            .addresses
            .into_iter()
            .next()
            .ok_or(WalletError::ApiError { message: "No account created".to_string() })?;

        let address = EvmAddress::from_str(&address_str).map_err(|e| WalletError::ApiError {
            message: format!("Invalid address format: {}", e),
        })?;

        let path = format!("m/44'/60'/{}'/0/{}", wallet_index, 0);
        let account_id = format!("{}_{}", self.wallet_id, wallet_index);

        let new_account = TurnkeyAccount {
            address: address_str.clone(),
            wallet_id: self.wallet_id.clone(),
            account_id,
            path,
            curve: "CURVE_SECP256K1".to_string(),
            path_format: "PATH_FORMAT_BIP32".to_string(),
            address_format: "ADDRESS_FORMAT_ETHEREUM".to_string(),
            public_key: "".to_string(), // Will be populated when needed
            created_at: None,
            updated_at: None,
        };

        {
            let mut accounts = self.accounts.lock().await;
            accounts.entry(wallet_index).or_insert(new_account);
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
        chain_id: &ChainId,
    ) -> Result<Signature, WalletError> {
        info!(
            "Turnkey sign_transaction called - wallet_index: {}, chain_id: {}",
            wallet_index, chain_id
        );

        let accounts = self.accounts.lock().await;
        let account = accounts.get(&wallet_index).ok_or_else(|| {
            error!("Turnkey sign_transaction: wallet not found for index {}", wallet_index);
            WalletError::WalletNotFound { index: wallet_index }
        })?;
        let account_id = account.account_id.clone();
        let account_address = account.address.clone();
        drop(accounts);

        info!("Turnkey sign_transaction: using account {} ({})", account_id, account_address);

        let unsigned_transaction_hex = match transaction {
            TypedTransaction::Eip1559(tx) => {
                let unsigned_tx = alloy::consensus::TxEip1559 {
                    chain_id: tx.chain_id,
                    nonce: tx.nonce,
                    max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                    max_fee_per_gas: tx.max_fee_per_gas,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input.clone(),
                    access_list: tx.access_list.clone(),
                };

                let encoded = alloy_rlp::encode(&unsigned_tx);
                format!("0x02{}", hex::encode(&encoded))
            }
            TypedTransaction::Legacy(tx) => {
                let unsigned_tx = alloy::consensus::TxLegacy {
                    chain_id: tx.chain_id,
                    nonce: tx.nonce,
                    gas_price: tx.gas_price,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input.clone(),
                };

                let encoded = alloy_rlp::encode(&unsigned_tx);
                format!("0x{}", hex::encode(&encoded))
            }
            TypedTransaction::Eip2930(tx) => {
                let unsigned_tx = alloy::consensus::TxEip2930 {
                    chain_id: tx.chain_id,
                    nonce: tx.nonce,
                    gas_price: tx.gas_price,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input.clone(),
                    access_list: tx.access_list.clone(),
                };

                let encoded = alloy_rlp::encode(&unsigned_tx);
                format!("0x01{}", hex::encode(&encoded))
            }
            TypedTransaction::Eip4844(tx_variant) => {
                info!("Turnkey sign_transaction: handling EIP-4844 blob transaction");

                let tx = match tx_variant {
                    alloy::consensus::TxEip4844Variant::TxEip4844WithSidecar(tx_with_sidecar) => {
                        &tx_with_sidecar.tx
                    }
                    alloy::consensus::TxEip4844Variant::TxEip4844(tx) => tx,
                };

                let unsigned_tx = alloy::consensus::TxEip4844 {
                    chain_id: tx.chain_id,
                    nonce: tx.nonce,
                    max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
                    max_fee_per_gas: tx.max_fee_per_gas,
                    gas_limit: tx.gas_limit,
                    to: tx.to,
                    value: tx.value,
                    input: tx.input.clone(),
                    access_list: tx.access_list.clone(),
                    blob_versioned_hashes: tx.blob_versioned_hashes.clone(),
                    max_fee_per_blob_gas: tx.max_fee_per_blob_gas,
                };

                let encoded = alloy_rlp::encode(&unsigned_tx);
                format!("0x03{}", hex::encode(&encoded))
            }
            _ => {
                error!("Turnkey sign_transaction: unsupported transaction type");
                return Err(WalletError::UnsupportedTransactionType {
                    tx_type: "Unsupported transaction type for Turnkey".to_string(),
                });
            }
        };

        info!("Turnkey sign_transaction: unsigned transaction hex: {}", unsigned_transaction_hex);

        let timestamp = Self::get_timestamp_ms();
        let request = serde_json::json!({
            "type": "ACTIVITY_TYPE_SIGN_TRANSACTION_V2",
            "organizationId": self.organization_id,
            "timestampMs": timestamp,
            "parameters": {
                "signWith": account_address,
                "unsignedTransaction": unsigned_transaction_hex,
                "type": "TRANSACTION_TYPE_ETHEREUM"
            }
        });

        let body = serde_json::to_string(&request).map_err(|e| {
            error!("Turnkey sign_transaction: failed to serialize request: {}", e);
            WalletError::ApiError { message: format!("Failed to serialize request: {}", e) }
        })?;

        info!("Turnkey sign_transaction: request body: {}", body);

        let stamp = self.create_stamp(&body)?;

        info!("Turnkey sign_transaction: sending request to /submit/sign_transaction endpoint");

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/submit/sign_transaction")
            .header("X-Stamp", &stamp)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status();
        info!(
            "Turnkey sign_transaction: response status: {}, headers: {:?}",
            status,
            response.headers()
        );

        if !status.is_success() {
            let error_text = response.text().await?;
            error!(
                "Turnkey sign_transaction: API error - status: {}, body: {}",
                status, error_text
            );
            return Err(WalletError::ApiError {
                message: format!("Turnkey API error: {}", error_text),
            });
        }

        let result: TurnkeyCreateAccountResponse = response.json().await?;

        info!("Turnkey sign_transaction: activity status: {}", result.activity.status);

        if result.activity.status != "ACTIVITY_STATUS_COMPLETED" {
            error!(
                "Turnkey sign_transaction: signing failed with status: {}",
                result.activity.status
            );
            return Err(WalletError::ApiError {
                message: format!(
                    "Transaction signing failed with status: {}",
                    result.activity.status
                ),
            });
        }

        let sign_result =
            result.activity.result.and_then(|r| r.sign_transaction_result).ok_or_else(|| {
                error!("Turnkey sign_transaction: no sign result in response");
                WalletError::ApiError { message: "No sign result in response".to_string() }
            })?;

        info!("Turnkey sign_transaction: signed transaction: {}", sign_result.signed_transaction);

        let signed_tx_hex = sign_result.signed_transaction.trim_start_matches("0x");
        let tx_bytes = hex::decode(signed_tx_hex).map_err(|e| {
            error!("Turnkey sign_transaction: failed to decode signed transaction hex: {}", e);
            WalletError::ApiError { message: format!("Failed to decode signed transaction: {}", e) }
        })?;

        info!("Turnkey sign_transaction: decoded {} bytes from signed transaction", tx_bytes.len());

        let tx_envelope = TxEnvelope::decode(&mut tx_bytes.as_slice()).map_err(|e| {
            error!("Turnkey sign_transaction: failed to decode transaction envelope: {}", e);
            WalletError::ApiError {
                message: format!("Failed to decode transaction envelope: {}", e),
            }
        })?;

        info!("Turnkey sign_transaction: successfully decoded transaction envelope");

        let signature = match tx_envelope {
            TxEnvelope::Eip1559(signed_tx) => {
                info!("Turnkey sign_transaction: extracted signature from EIP1559 transaction");
                *signed_tx.signature()
            }
            TxEnvelope::Legacy(signed_tx) => {
                info!("Turnkey sign_transaction: extracted signature from Legacy transaction");
                *signed_tx.signature()
            }
            TxEnvelope::Eip2930(signed_tx) => {
                info!("Turnkey sign_transaction: extracted signature from EIP2930 transaction");
                *signed_tx.signature()
            }
            TxEnvelope::Eip4844(signed_tx) => {
                info!(
                    "Turnkey sign_transaction: extracted signature from EIP4844 blob transaction"
                );
                *signed_tx.signature()
            }
            _ => {
                error!("Turnkey sign_transaction: unsupported transaction envelope type");
                return Err(WalletError::UnsupportedTransactionType {
                    tx_type: "Unknown transaction envelope type".to_string(),
                });
            }
        };

        info!("Turnkey sign_transaction: successfully extracted signature");
        Ok(signature)
    }

    async fn sign_text(&self, wallet_index: u32, text: &str) -> Result<Signature, WalletError> {
        info!("Turnkey sign_text called - wallet_index: {}, text: '{}'", wallet_index, text);

        let accounts = self.accounts.lock().await;
        let account = accounts.get(&wallet_index).ok_or_else(|| {
            error!("Turnkey sign_text: wallet not found for index {}", wallet_index);
            WalletError::WalletNotFound { index: wallet_index }
        })?;
        let account_id = account.account_id.clone();
        let account_address = account.address.clone();
        drop(accounts);

        info!("Turnkey sign_text: using account {} ({})", account_id, account_address);

        let message = format!("\x19Ethereum Signed Message:\n{}{}", text.len(), text);
        let message_hash = keccak256(message.as_bytes());

        info!("Turnkey sign_text: message='{}', hash=0x{}", message, hex::encode(message_hash));

        let timestamp = Self::get_timestamp_ms();
        let request = serde_json::json!({
            "type": "ACTIVITY_TYPE_SIGN_RAW_PAYLOAD_V2",
            "organizationId": self.organization_id,
            "timestampMs": timestamp,
            "parameters": {
                "signWith": account_address,
                "payload": format!("0x{}", hex::encode(message_hash)),
                "encoding": "PAYLOAD_ENCODING_HEXADECIMAL",
                "hashFunction": "HASH_FUNCTION_NO_OP"
            }
        });

        let body = serde_json::to_string(&request).map_err(|e| {
            error!("Turnkey sign_text: failed to serialize request: {}", e);
            WalletError::ApiError { message: format!("Failed to serialize request: {}", e) }
        })?;

        info!("Turnkey sign_text: request body: {}", body);

        let stamp = self.create_stamp(&body)?;

        info!("Turnkey sign_text: sending request to /submit/sign_raw_payload endpoint");

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/submit/sign_raw_payload")
            .header("X-Stamp", &stamp)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status();
        info!("Turnkey sign_text: response status: {}, headers: {:?}", status, response.headers());

        if !status.is_success() {
            let error_text = response.text().await?;
            error!("Turnkey sign_text: API error - status: {}, body: {}", status, error_text);
            return Err(WalletError::ApiError {
                message: format!("Turnkey API error: {}", error_text),
            });
        }

        let response_text = response.text().await?;
        info!("Turnkey sign_text: raw response body: {}", response_text);

        let result: TurnkeyCreateAccountResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                error!("Turnkey sign_text: failed to parse response '{}': {}", response_text, e);
                WalletError::ApiError { message: format!("Failed to parse response: {}", e) }
            })?;

        info!("Turnkey sign_text: activity status: {}", result.activity.status);

        if result.activity.status != "ACTIVITY_STATUS_COMPLETED" {
            error!("Turnkey sign_text: signing failed with status: {}", result.activity.status);
            return Err(WalletError::ApiError {
                message: format!("Text signing failed with status: {}", result.activity.status),
            });
        }

        let sign_result =
            result.activity.result.and_then(|r| r.sign_raw_payload_result).ok_or_else(|| {
                error!("Turnkey sign_text: no sign result in response");
                WalletError::ApiError { message: "No sign result in response".to_string() }
            })?;

        info!(
            "Turnkey sign_text: signature result: r={}, s={}, v={}",
            sign_result.r, sign_result.s, sign_result.v
        );

        let r_bytes = hex::decode(sign_result.r.trim_start_matches("0x")).map_err(|e| {
            error!("Turnkey sign_text: failed to decode r value '{}': {}", sign_result.r, e);
            WalletError::ApiError { message: format!("Failed to decode r value: {}", e) }
        })?;

        let s_bytes = hex::decode(sign_result.s.trim_start_matches("0x")).map_err(|e| {
            error!("Turnkey sign_text: failed to decode s value '{}': {}", sign_result.s, e);
            WalletError::ApiError { message: format!("Failed to decode s value: {}", e) }
        })?;

        let v_value =
            u64::from_str_radix(sign_result.v.trim_start_matches("0x"), 16).map_err(|e| {
                error!("Turnkey sign_text: failed to decode v value '{}': {}", sign_result.v, e);
                WalletError::ApiError { message: format!("Failed to decode v value: {}", e) }
            })?;

        let r_bytes_32: [u8; 32] = r_bytes.try_into().map_err(|_| {
            error!("Turnkey sign_text: r value is not 32 bytes");
            WalletError::ApiError { message: "r value is not 32 bytes".to_string() }
        })?;

        let s_bytes_32: [u8; 32] = s_bytes.try_into().map_err(|_| {
            error!("Turnkey sign_text: s value is not 32 bytes");
            WalletError::ApiError { message: "s value is not 32 bytes".to_string() }
        })?;

        let signature = Signature::new(
            alloy::primitives::U256::from_be_bytes(r_bytes_32),
            alloy::primitives::U256::from_be_bytes(s_bytes_32),
            v_value != 0,
        );

        info!("Turnkey sign_text: successfully parsed signature");
        Ok(signature)
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
    ) -> Result<Signature, WalletError> {
        let accounts = self.accounts.lock().await;
        let _ = accounts
            .get(&wallet_index)
            .ok_or(WalletError::WalletNotFound { index: wallet_index })?;

        let typed_data_json = serde_json::to_string(typed_data).map_err(|e| {
            WalletError::ApiError { message: format!("Failed to serialize typed data: {}", e) }
        })?;

        let timestamp = Self::get_timestamp_ms();
        let accounts = self.accounts.lock().await;
        let account = accounts.get(&wallet_index).ok_or_else(|| {
            error!("Turnkey sign_typed_data: wallet not found for index {}", wallet_index);
            WalletError::WalletNotFound { index: wallet_index }
        })?;
        let account_address = account.address.clone();
        drop(accounts);

        let request = serde_json::json!({
            "type": "ACTIVITY_TYPE_SIGN_RAW_PAYLOAD_V2",
            "organizationId": self.organization_id,
            "timestampMs": timestamp,
            "parameters": {
                "signWith": account_address,
                "payload": typed_data_json,
                "encoding": "PAYLOAD_ENCODING_EIP712",
                "hashFunction": "HASH_FUNCTION_NOT_APPLICABLE"
            }
        });

        let body = serde_json::to_string(&request).map_err(|e| WalletError::ApiError {
            message: format!("Failed to serialize request: {}", e),
        })?;

        let stamp = self.create_stamp(&body)?;

        let response = self
            .client
            .post("https://api.turnkey.com/public/v1/submit/sign_raw_payload")
            .header("X-Stamp", &stamp)
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

        info!(
            "Turnkey sign_typed_data: signature result: r={}, s={}, v={}",
            sign_result.r, sign_result.s, sign_result.v
        );

        let r_bytes = hex::decode(sign_result.r.trim_start_matches("0x")).map_err(|e| {
            error!("Turnkey sign_typed_data: failed to decode r value '{}': {}", sign_result.r, e);
            WalletError::ApiError { message: format!("Failed to decode r value: {}", e) }
        })?;

        let s_bytes = hex::decode(sign_result.s.trim_start_matches("0x")).map_err(|e| {
            error!("Turnkey sign_typed_data: failed to decode s value '{}': {}", sign_result.s, e);
            WalletError::ApiError { message: format!("Failed to decode s value: {}", e) }
        })?;

        let v_value =
            u64::from_str_radix(sign_result.v.trim_start_matches("0x"), 16).map_err(|e| {
                error!(
                    "Turnkey sign_typed_data: failed to decode v value '{}': {}",
                    sign_result.v, e
                );
                WalletError::ApiError { message: format!("Failed to decode v value: {}", e) }
            })?;

        let r_bytes_32: [u8; 32] = r_bytes.try_into().map_err(|_| {
            error!("Turnkey sign_typed_data: r value is not 32 bytes");
            WalletError::ApiError { message: "r value is not 32 bytes".to_string() }
        })?;

        let s_bytes_32: [u8; 32] = s_bytes.try_into().map_err(|_| {
            error!("Turnkey sign_typed_data: s value is not 32 bytes");
            WalletError::ApiError { message: "s value is not 32 bytes".to_string() }
        })?;

        let signature = Signature::new(
            alloy::primitives::U256::from_be_bytes(r_bytes_32),
            alloy::primitives::U256::from_be_bytes(s_bytes_32),
            v_value != 0,
        );

        info!("Turnkey sign_typed_data: successfully parsed signature");
        Ok(signature)
    }

    fn supports_blobs(&self) -> bool {
        true
    }
}
