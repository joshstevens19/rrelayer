use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerChainId, WalletManagerTrait};
use crate::yaml::FireblocksSigningProviderConfig;
use alloy::consensus::{SignableTransaction, TypedTransaction};
use alloy::dyn_abi::TypedData;
use alloy::primitives::{keccak256, Signature, B256, U256};
use async_trait::async_trait;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::{debug, info};

#[derive(Debug, Serialize, Deserialize)]
struct FireblocksClaims {
    uri: String,
    nonce: u64,
    iat: u64,
    exp: u64,
    sub: String,
    #[serde(rename = "bodyHash")]
    body_hash: String,
}

#[derive(Debug)]
pub struct FireblocksWalletManager {
    config: FireblocksSigningProviderConfig,
    http_client: Client,
    // (wallet_index, chain_id) -> (vault_account_id, address)
    wallet_cache: Mutex<HashMap<(u32, ChainId), (String, EvmAddress)>>,
    // chain_id -> fireblocks_asset_id
    chain_asset_mapping: HashMap<ChainId, &'static str>,
}

impl FireblocksWalletManager {
    fn get_chain_asset_mapping() -> HashMap<ChainId, &'static str> {
        let mut mapping = HashMap::new();

        mapping.insert(ChainId::new(1), "ETH");
        mapping.insert(ChainId::new(11155111), "ETH_TEST5");
        mapping.insert(ChainId::new(17000), "ETH_TEST5");

        mapping.insert(ChainId::new(137), "MATIC_POLYGON");
        mapping.insert(ChainId::new(80001), "MATIC_POLYGON");
        mapping.insert(ChainId::new(80002), "MATIC_POLYGON");

        mapping.insert(ChainId::new(42161), "ETH-AETH");
        mapping.insert(ChainId::new(421614), "ETH_TEST5");

        mapping.insert(ChainId::new(10), "ETH-OPT");
        mapping.insert(ChainId::new(11155420), "ETH_TEST5");

        mapping.insert(ChainId::new(8453), "BASECHAIN_ETH");
        mapping.insert(ChainId::new(84532), "ETH_TEST5");

        mapping.insert(ChainId::new(56), "BNB_BSC");
        mapping.insert(ChainId::new(97), "BNB_BSC");

        mapping.insert(ChainId::new(43114), "AVAX");
        mapping.insert(ChainId::new(43113), "AVAX");

        mapping.insert(ChainId::new(250), "FYM");
        mapping.insert(ChainId::new(100), "xDAI");
        mapping.insert(ChainId::new(42220), "CELO");
        mapping.insert(ChainId::new(59144), "LINEA");

        mapping.insert(ChainId::new(1284), "GLMR_GLMR");
        mapping.insert(ChainId::new(1285), "MOVR_MOVR");
        mapping.insert(ChainId::new(1313161554), "AURORA_DEV");
        mapping.insert(ChainId::new(592), "ASTR_ASTR");
        mapping.insert(ChainId::new(88888), "CHZ_CHZ2");
        mapping.insert(ChainId::new(9001), "EVMOS");
        mapping.insert(ChainId::new(2222), "KAVA");
        mapping.insert(ChainId::new(248), "OAS");
        mapping.insert(ChainId::new(30), "RBTC");
        mapping.insert(ChainId::new(106), "VLX_VLX");
        mapping.insert(ChainId::new(50), "XDC");
        mapping.insert(ChainId::new(51), "XDC");
        mapping.insert(ChainId::new(19), "SGB");
        mapping.insert(ChainId::new(7700), "CANTO");
        mapping.insert(ChainId::new(128), "HT_CHAIN");
        mapping.insert(ChainId::new(148), "SMR_SMR");
        mapping.insert(ChainId::new(10000), "SMARTBCH");

        mapping
    }

    fn get_asset_id_for_chain(&self, chain_id: &ChainId) -> Result<&'static str, WalletError> {
        self.chain_asset_mapping.get(chain_id).copied().ok_or_else(|| {
            WalletError::ConfigurationError {
                message: format!(
                    "Fireblocks does not support chain ID {}. Supported chains: {}",
                    chain_id.u64(),
                    self.chain_asset_mapping
                        .keys()
                        .map(|id| id.u64().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        })
    }

    pub async fn new(config: FireblocksSigningProviderConfig) -> Result<Self, WalletError> {
        config.validate().map_err(|e| WalletError::ConfigurationError { message: e })?;

        Ok(Self {
            config,
            http_client: Client::new(),
            wallet_cache: Mutex::new(HashMap::new()),
            chain_asset_mapping: Self::get_chain_asset_mapping(),
        })
    }

    fn generate_jwt_token(&self, uri: &str, body: &Option<Value>) -> Result<String, WalletError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| WalletError::GenericSignerError(format!("System time error: {}", e)))?
            .as_secs();

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| WalletError::GenericSignerError(format!("System time error: {}", e)))?
            .as_micros() as u64;

        let body_bytes =
            if let Some(body) = body { body.to_string().into_bytes() } else { Vec::new() };

        let mut hasher = Sha256::new();
        hasher.update(&body_bytes);
        let body_hash = hex::encode(hasher.finalize());

        let claims = FireblocksClaims {
            uri: uri.to_string(),
            nonce,
            iat: now,
            // Token expires in 29 seconds (Fireblocks requires under 30s)
            exp: now + 29,
            sub: self.config.api_key.clone(),
            body_hash,
        };

        let header = Header::new(Algorithm::RS256);

        let private_key_content =
            std::fs::read_to_string(&self.config.private_key_path).map_err(|e| {
                WalletError::ConfigurationError {
                    message: format!(
                        "Failed to read private key file {}: {}",
                        self.config.private_key_path, e
                    ),
                }
            })?;

        let encoding_key =
            EncodingKey::from_rsa_pem(private_key_content.as_bytes()).map_err(|e| {
                WalletError::ConfigurationError {
                    message: format!("Invalid private key format: {}", e),
                }
            })?;

        encode(&header, &claims, &encoding_key)
            .map_err(|e| WalletError::GenericSignerError(format!("JWT generation failed: {}", e)))
    }

    async fn make_api_request(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<Value>,
    ) -> Result<Value, WalletError> {
        let url = format!("{}{}", self.config.get_base_url(), endpoint);

        let mut request = match method {
            "GET" => self.http_client.get(&url),
            "POST" => self.http_client.post(&url),
            "PUT" => self.http_client.put(&url),
            "DELETE" => self.http_client.delete(&url),
            _ => {
                return Err(WalletError::GenericSignerError("Unsupported HTTP method".to_string()))
            }
        };

        let jwt_token = self.generate_jwt_token(endpoint, &body)?;

        request = request
            .header("X-API-Key", &self.config.api_key)
            .header("Authorization", format!("Bearer {}", jwt_token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await.map_err(WalletError::NetworkError)?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(WalletError::ApiError {
                message: format!("Fireblocks API error ({}): {}", status, error_text),
            });
        }

        let json_response: Value = response.json().await.map_err(|e| {
            WalletError::GenericSignerError(format!("Failed to parse JSON response: {}", e))
        })?;

        Ok(json_response)
    }

    async fn get_or_create_vault_account(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<(String, EvmAddress), WalletError> {
        let cache_key = (wallet_index, *chain_id);

        {
            let cache = self.wallet_cache.lock().await;
            if let Some(&(ref vault_id, address)) = cache.get(&cache_key) {
                return Ok((vault_id.clone(), address));
            }
        }

        // Generate vault account name (chain-specific and instance-specific)
        // This ensures different RRelayer instances, and different chains use isolated vault accounts
        let vault_name =
            format!("rrelayer-{}-wallet-{}-{}", self.config.identity, chain_id.u64(), wallet_index);

        let response = self.make_api_request("GET", "/v1/vault/accounts_paged", None).await?;

        let vault_accounts =
            response.get("accounts").and_then(|accounts| accounts.as_array()).ok_or_else(|| {
                WalletError::GenericSignerError(
                    "Invalid vault accounts response - missing 'accounts' field".to_string(),
                )
            })?;

        let existing_vault = vault_accounts.iter().find(|va| {
            va.get("name").and_then(|n| n.as_str()).is_some_and(|name| name == vault_name)
        });

        let vault_id = if let Some(vault) = existing_vault {
            let vault_id = vault.get("id").and_then(|id| id.as_str()).ok_or_else(|| {
                WalletError::GenericSignerError("Vault account missing ID".to_string())
            })?;
            vault_id.to_string()
        } else {
            info!("Creating new vault account: {}", vault_name);
            let create_request = json!({
                "name": vault_name,
                "hiddenOnUI": self.config.hidden_on_ui.unwrap_or(true)
            });

            let response =
                self.make_api_request("POST", "/v1/vault/accounts", Some(create_request)).await?;
            response
                .get("id")
                .and_then(|id| id.as_str())
                .ok_or_else(|| {
                    WalletError::GenericSignerError("Failed to get vault account ID".to_string())
                })?
                .to_string()
        };

        let asset_address = self.get_or_create_chain_asset(&vault_id, chain_id).await?;

        let address = EvmAddress::from_str(&asset_address).map_err(|e| {
            WalletError::GenericSignerError(format!("Invalid address format: {}", e))
        })?;

        {
            let mut cache = self.wallet_cache.lock().await;
            cache.insert(cache_key, (vault_id.clone(), address));
        }

        info!(
            "Vault account {} ready for wallet {} on chain {} at address {}",
            vault_id,
            wallet_index,
            chain_id.u64(),
            address
        );

        Ok((vault_id, address))
    }

    async fn get_or_create_chain_asset(
        &self,
        vault_id: &str,
        chain_id: &ChainId,
    ) -> Result<String, WalletError> {
        let asset_id = self.get_asset_id_for_chain(chain_id)?;

        let check_endpoint = format!("/v1/vault/accounts/{}/{}", vault_id, asset_id);
        match self.make_api_request("GET", &check_endpoint, None).await {
            Ok(existing_asset) => {
                return if let Some(address) =
                    existing_asset.get("address").and_then(|addr| addr.as_str())
                {
                    Ok(address.to_string())
                } else {
                    let addresses_endpoint =
                        format!("/v1/vault/accounts/{}/{}/addresses_paginated", vault_id, asset_id);

                    match self.make_api_request("GET", &addresses_endpoint, None).await {
                        Ok(addresses_response) => {
                            if let Some(addresses) = addresses_response
                                .get("addresses")
                                .and_then(|addrs| addrs.as_array())
                            {
                                if let Some(first_address) = addresses.first() {
                                    if let Some(address) =
                                        first_address.get("address").and_then(|addr| addr.as_str())
                                    {
                                        info!(
                                            "Found address for {} asset in vault {}: {}",
                                            asset_id, vault_id, address
                                        );
                                        return Ok(address.to_string());
                                    }
                                }
                            }
                            Err(WalletError::GenericSignerError(format!(
                                "{} asset exists but has no addresses",
                                asset_id
                            )))
                        }
                        Err(_) => Err(WalletError::GenericSignerError(format!(
                            "{} asset exists but failed to get addresses",
                            asset_id
                        ))),
                    }
                }
            }
            Err(WalletError::ApiError { message })
                if message.contains("404") || message.contains("not found") =>
            {
                info!("Asset {} not found in vault {}, will create it", asset_id, vault_id);
            }
            Err(e) => {
                return Err(e);
            }
        }

        info!("Creating {} asset for vault {} (chain ID: {})", asset_id, vault_id, chain_id.u64());
        let create_endpoint = format!("/v1/vault/accounts/{}/{}", vault_id, asset_id);

        let response = self.make_api_request("POST", &create_endpoint, None).await?;
        response
            .get("address")
            .and_then(|addr| addr.as_str())
            .ok_or_else(|| {
                WalletError::GenericSignerError(format!("Failed to get {} asset address", asset_id))
            })
            .map(|s| s.to_string())
    }

    async fn sign_hash(
        &self,
        wallet_index: u32,
        hash: &B256,
        chain_id: &ChainId,
    ) -> Result<Signature, WalletError> {
        let (vault_id, _address) = self.get_or_create_vault_account(wallet_index, chain_id).await?;
        let asset_id = self.get_asset_id_for_chain(chain_id)?;

        let sign_request = json!({
            "operation": "RAW",
            "source": {
                "type": "VAULT_ACCOUNT",
                "id": vault_id
            },
            "assetId": asset_id,
            "note": format!("RRelayer signature for wallet {} on chain {}", wallet_index, chain_id.u64()),
            "extraParameters": {
                "rawMessageData": {
                    "messages": [{
                        "content": hex::encode(hash.as_slice()),
                        "bip44addressIndex": 0,
                        "bip44change": 0
                    }]
                }
            }
        });

        let response =
            self.make_api_request("POST", "/v1/transactions", Some(sign_request)).await?;

        let tx_id = response.get("id").and_then(|id| id.as_str()).ok_or_else(|| {
            WalletError::GenericSignerError("Failed to get transaction ID".to_string())
        })?;

        self.wait_for_signature_completion(tx_id).await
    }

    async fn wait_for_signature_completion(&self, tx_id: &str) -> Result<Signature, WalletError> {
        // 15 seconds with 250ms intervals
        const MAX_RETRIES: u32 = 60;
        const RETRY_INTERVAL: tokio::time::Duration = tokio::time::Duration::from_millis(250);

        for attempt in 0..MAX_RETRIES {
            let endpoint = format!("/v1/transactions/{}", tx_id);
            let tx_response = self.make_api_request("GET", &endpoint, None).await?;

            let status = tx_response.get("status").and_then(|s| s.as_str()).ok_or_else(|| {
                WalletError::GenericSignerError("Transaction missing status".to_string())
            })?;

            match status {
                "COMPLETED" => {
                    info!("Fireblocks Transaction {} completed successfully", tx_id);
                    return self.extract_signature_from_response(&tx_response).await;
                }
                "FAILED" | "REJECTED" | "CANCELLED" => {
                    return Err(WalletError::GenericSignerError(format!(
                        "Transaction {} failed with status: {}",
                        tx_id, status
                    )));
                }
                _ => {
                    debug!(
                        "Transaction {} still pending (status: {}), attempt {}/{}",
                        tx_id,
                        status,
                        attempt + 1,
                        MAX_RETRIES
                    );
                    tokio::time::sleep(RETRY_INTERVAL).await;
                }
            }
        }

        Err(WalletError::GenericSignerError(format!(
            "Transaction {} timed out after {} attempts",
            tx_id, MAX_RETRIES
        )))
    }

    async fn extract_signature_from_response(
        &self,
        tx_response: &Value,
    ) -> Result<Signature, WalletError> {
        let signed_messages =
            tx_response.get("signedMessages").and_then(|sm| sm.as_array()).ok_or_else(|| {
                WalletError::GenericSignerError("No signed messages in response".to_string())
            })?;

        let signature_data = signed_messages.first().ok_or_else(|| {
            WalletError::GenericSignerError("No signature data found".to_string())
        })?;

        let sig_value = signature_data.get("signature").ok_or_else(|| {
            WalletError::GenericSignerError("No signature in response".to_string())
        })?;

        let sig_hex = sig_value
            .get("fullSig")
            .and_then(|s| s.as_str())
            .or_else(|| sig_value.as_str())
            .ok_or_else(|| {
                WalletError::GenericSignerError("Invalid signature format".to_string())
            })?;

        let sig_bytes = hex::decode(sig_hex.trim_start_matches("0x")).map_err(|e| {
            WalletError::GenericSignerError(format!("Invalid signature hex: {}", e))
        })?;

        if sig_bytes.len() != 64 {
            return Err(WalletError::GenericSignerError(format!(
                "Invalid signature length: expected 64 bytes, got {}",
                sig_bytes.len()
            )));
        }

        let r = U256::from_be_slice(&sig_bytes[0..32]);
        let s = U256::from_be_slice(&sig_bytes[32..64]);

        let recovery_id = signature_data
            .get("signature")
            .and_then(|sig| sig.get("v"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let parity = (recovery_id % 2) != 0;

        Ok(Signature::new(r, s, parity))
    }
}

#[async_trait]
impl WalletManagerTrait for FireblocksWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: WalletManagerChainId,
    ) -> Result<EvmAddress, WalletError> {
        let (_vault_id, address) =
            self.get_or_create_vault_account(wallet_index, chain_id.main()).await?;
        Ok(address)
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: WalletManagerChainId,
    ) -> Result<EvmAddress, WalletError> {
        let (_vault_id, address) =
            self.get_or_create_vault_account(wallet_index, chain_id.main()).await?;
        Ok(address)
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: WalletManagerChainId,
    ) -> Result<Signature, WalletError> {
        let tx_hash = transaction.signature_hash();
        self.sign_hash(wallet_index, &tx_hash, chain_id.main()).await
    }

    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
        chain_id: WalletManagerChainId,
    ) -> Result<Signature, WalletError> {
        let message = format!("\x19Ethereum Signed Message:\n{}{}", text.len(), text);
        let hash = keccak256(message.as_bytes());
        self.sign_hash(wallet_index, &hash, chain_id.main()).await
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
        chain_id: WalletManagerChainId,
    ) -> Result<Signature, WalletError> {
        let hash = typed_data.eip712_signing_hash()?;
        self.sign_hash(wallet_index, &hash, chain_id.main()).await
    }

    fn supports_blobs(&self) -> bool {
        true
    }
}
