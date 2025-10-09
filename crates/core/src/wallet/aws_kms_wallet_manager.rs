use crate::common_types::EvmAddress;
use crate::network::ChainId;
use crate::wallet::{WalletError, WalletManagerTrait};
use crate::yaml::AwsKmsSigningProviderConfig;
use alloy::consensus::TypedTransaction;
use alloy::dyn_abi::TypedData;
use alloy::network::TxSigner;
use alloy::primitives::Signature;
use alloy::signers::{aws::AwsSigner, Signer};
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_kms::{
    types::{KeySpec, KeyUsageType, Tag},
    Client,
};
use aws_sdk_sts::Client as StsClient;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

#[derive(Clone, Debug)]
pub struct KeyPlan {
    pub description: String,
    pub alias: Option<String>,
    pub tags: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct AwsKmsWalletManager {
    config: AwsKmsSigningProviderConfig,
    alias: String,
    signers: Arc<RwLock<HashMap<(u32, u64), AwsSigner>>>,
}

impl AwsKmsWalletManager {
    pub fn new(config: AwsKmsSigningProviderConfig) -> Self {
        Self {
            alias: config.danger_override_alias.clone().unwrap_or("rrelayer".to_string()),
            config,
            signers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn build_alias(&self, wallet_index: u32, chain_id: &ChainId) -> String {
        format!("alias/{}-wallet-{}-{}", self.alias, wallet_index, chain_id)
    }

    async fn build_aws_config(&self) -> aws_config::SdkConfig {
        let mut config_loader = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(self.config.region.clone()));

        if let Some(endpoint_url) = &self.config.endpoint_url {
            config_loader = config_loader.endpoint_url(endpoint_url);
        }

        config_loader.load().await
    }

    async fn get_or_create_key_id(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<String, WalletError> {
        self.validate_aws_config().await?;
        match self.find_key_by_alias(wallet_index, chain_id).await {
            Ok(key_id) => {
                return Ok(key_id);
            }
            Err(e) => {
                debug!("AWS KMS: No existing key found: {}", e);
            }
        }

        info!("AWS KMS: Creating new key for wallet_index {}", wallet_index);
        let key_id = self.create_key_for_wallet_index(wallet_index, chain_id).await?;
        info!("AWS KMS: Successfully created new key: {}", key_id);
        Ok(key_id)
    }

    async fn validate_aws_config(&self) -> Result<(), WalletError> {
        let aws_config = self.build_aws_config().await;

        let sts = StsClient::new(&aws_config);
        match sts.get_caller_identity().send().await {
            Ok(_) => Ok(()),
            Err(e) => {
                let error_msg = format!(
                    "AWS KMS authentication failed. Please ensure AWS credentials are properly configured. \
                    Error: {}. \
                    Required: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, or an IAM role with KMS permissions.",
                    e
                );
                Err(WalletError::AuthenticationError { message: error_msg })
            }
        }
    }

    async fn find_key_by_alias(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<String, WalletError> {
        let aws_config = self.build_aws_config().await;

        let kms = Client::new(&aws_config);
        let expected_alias = self.build_alias(wallet_index, chain_id);

        let alias_response = kms.list_aliases().send().await.map_err(|e| {
            WalletError::ApiError { message: format!("Failed to list aliases: {}", e) }
        })?;

        let alias_list = alias_response.aliases();
        for alias in alias_list {
            if let Some(alias_name) = alias.alias_name() {
                if alias_name == expected_alias {
                    if let Some(target_key_id) = alias.target_key_id() {
                        return Ok(target_key_id.to_string());
                    }
                }
            }
        }

        Err(WalletError::ApiError {
            message: format!("No KMS key found for alias: {}", expected_alias),
        })
    }

    async fn create_key_for_wallet_index(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<String, WalletError> {
        let plan = KeyPlan {
            description: format!(
                "ECC_SECG_P256K1 signing key - wallet_{}_chain_{}",
                wallet_index, chain_id
            ),
            alias: Some(self.build_alias(wallet_index, chain_id)),
            tags: vec![], // No tags needed, alias is the identifier
        };

        match self.create_keys(vec![plan]).await {
            Ok(keys) => {
                if let Some(key_id) = keys.into_iter().next() {
                    Ok(key_id)
                } else {
                    let error = WalletError::ApiError {
                        message: "Failed to create KMS key - no key ID returned".to_string(),
                    };
                    Err(error)
                }
            }
            Err(e) => Err(e),
        }
    }

    async fn get_or_initialize_signer(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<AwsSigner, WalletError> {
        let chain_id_u64 = chain_id.u64();
        let cache_key = (wallet_index, chain_id_u64);

        {
            let signers = self.signers.read().await;
            if let Some(signer) = signers.get(&cache_key) {
                return Ok(signer.clone());
            }
        }

        let key_id = self.get_or_create_key_id(wallet_index, chain_id).await?;
        let signer = self.initialize_aws_kms_signer(&key_id, Some(chain_id_u64)).await?;

        {
            let mut signers = self.signers.write().await;
            signers.insert(cache_key, signer.clone());
        }

        Ok(signer)
    }

    async fn initialize_aws_kms_signer(
        &self,
        key_id: &str,
        chain_id: Option<u64>,
    ) -> Result<AwsSigner, WalletError> {
        let aws_config = self.build_aws_config().await;

        let client = Client::new(&aws_config);

        let signer = AwsSigner::new(client, key_id.to_string(), chain_id).await.map_err(|e| {
            WalletError::ApiError { message: format!("Failed to initialize AWS KMS signer: {}", e) }
        })?;

        Ok(signer)
    }

    fn normalize_principal_arn(caller_arn: &str) -> String {
        if let Some(rest) = caller_arn.strip_prefix("arn:aws:sts::") {
            if let Some(pos) = rest.find(":assumed-role/") {
                let (account_id, after) = rest.split_at(pos);
                let parts: Vec<&str> =
                    after.trim_start_matches(":assumed-role/").split('/').collect();
                if let Some(role_name) = parts.first() {
                    return format!("arn:aws:iam::{}:role/{}", account_id, role_name);
                }
            }
        }
        caller_arn.to_string()
    }

    fn build_key_policy(account_id: &str, admin_principal_arn: &str) -> String {
        let policy = json!({
            "Version": "2012-10-17",
            "Id": "key-default-1",
            "Statement": [
                {
                    "Sid": "AllowRootAccountAccess",
                    "Effect": "Allow",
                    "Principal": { "AWS": format!("arn:aws:iam::{}:root", account_id) },
                    "Action": [
                        "kms:DescribeKey",
                        "kms:ListAliases",
                        "kms:ListKeyPolicies",
                        "kms:GetKeyPolicy",
                        "kms:PutKeyPolicy",
                        "kms:CreateAlias",
                        "kms:DeleteAlias",
                        "kms:ScheduleKeyDeletion",
                        "kms:CancelKeyDeletion",
                        "kms:EnableKey",
                        "kms:DisableKey",
                        "kms:EnableKeyRotation",
                        "kms:DisableKeyRotation",
                        "kms:RevokeGrant",
                        "kms:RetireGrant"
                      ],
                    "Resource": "*"
                },
                {
                    "Sid": "AllowAdminPrincipalSelf",
                    "Effect": "Allow",
                    "Principal": { "AWS": admin_principal_arn },
                    "Action": "kms:*",
                    "Resource": "*"
                }
            ]
        });
        policy.to_string()
    }

    pub async fn create_keys(&self, plans: Vec<KeyPlan>) -> Result<Vec<String>, WalletError> {
        let aws_config = self.build_aws_config().await;

        let sts = StsClient::new(&aws_config);
        let who = sts.get_caller_identity().send().await.map_err(|e| WalletError::ApiError {
            message: format!("STS GetCallerIdentity failed: {}", e),
        })?;

        let account_id = who.account().ok_or_else(|| WalletError::ApiError {
            message: "No account id from STS".to_string(),
        })?;

        let caller_arn = who
            .arn()
            .ok_or_else(|| WalletError::ApiError { message: "No ARN from STS".to_string() })?;

        let admin_principal_arn = Self::normalize_principal_arn(caller_arn);
        let kms = Client::new(&aws_config);
        let policy = Self::build_key_policy(account_id, &admin_principal_arn);

        let mut created_keys = Vec::new();

        for plan in plans {
            let mut create_key_builder = kms
                .create_key()
                .description(&plan.description)
                .key_spec(KeySpec::EccSecgP256K1)
                .key_usage(KeyUsageType::SignVerify)
                .policy(policy.clone());

            for (k, v) in &plan.tags {
                let tag = Tag::builder().tag_key(k).tag_value(v).build().unwrap();
                create_key_builder = create_key_builder.tags(tag);
            }

            let out = create_key_builder.send().await.map_err(|e| {
                let service_error = e.into_service_error();
                let error_msg = format!("Creating key '{}': {}", plan.description, service_error);
                error!("AWS KMS: {}", error_msg);
                WalletError::ApiError { message: error_msg }
            })?;

            let key_id = out.key_metadata().map(|m| m.key_id()).ok_or_else(|| {
                WalletError::ApiError { message: "No key_id in CreateKey response".to_string() }
            })?;

            if let Some(alias) = &plan.alias {
                match kms.create_alias().alias_name(alias).target_key_id(key_id).send().await {
                    Ok(_) => {
                        info!("AWS KMS: Successfully created alias: {}", alias);
                    }
                    Err(e) => {
                        warn!(
                            "AWS KMS: Failed to create alias {} for key {}: {}",
                            alias, key_id, e
                        );
                    }
                }
            }

            created_keys.push(key_id.to_string());
        }

        Ok(created_keys)
    }

    pub async fn list_keys(&self) -> Result<Vec<(String, String)>, WalletError> {
        let aws_config = self.build_aws_config().await;

        let kms = Client::new(&aws_config);

        let response = kms.list_keys().send().await.map_err(|e| WalletError::ApiError {
            message: format!("Failed to list KMS keys: {}", e),
        })?;

        let mut keys = Vec::new();

        let key_list = response.keys();
        for key in key_list {
            if let (Some(key_id), Some(_key_arn)) = (key.key_id(), key.key_arn()) {
                if let Ok(desc) = kms.describe_key().key_id(key_id).send().await {
                    if let Some(metadata) = desc.key_metadata() {
                        if metadata.key_usage() == Some(&KeyUsageType::SignVerify) {
                            let description = metadata.description().unwrap_or("No description");
                            keys.push((key_id.to_string(), description.to_string()));
                        }
                    }
                }
            }
        }

        Ok(keys)
    }

    pub async fn list_aliases(&self) -> Result<Vec<(String, String)>, WalletError> {
        let aws_config = self.build_aws_config().await;

        let kms = Client::new(&aws_config);

        let response = kms.list_aliases().send().await.map_err(|e| WalletError::ApiError {
            message: format!("Failed to list KMS aliases: {}", e),
        })?;

        let mut aliases = Vec::new();

        let alias_list = response.aliases();
        for alias in alias_list {
            if let (Some(alias_name), Some(target_key_id)) =
                (alias.alias_name(), alias.target_key_id())
            {
                aliases.push((alias_name.to_string(), target_key_id.to_string()));
            }
        }

        Ok(aliases)
    }
}

#[async_trait]
impl WalletManagerTrait for AwsKmsWalletManager {
    async fn create_wallet(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;
        let address = EvmAddress::from(alloy::signers::Signer::address(&signer));
        info!("AWS KMS: Successfully created wallet with address: {}", address);
        Ok(address)
    }

    async fn get_address(
        &self,
        wallet_index: u32,
        chain_id: &ChainId,
    ) -> Result<EvmAddress, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;
        Ok(EvmAddress::from(alloy::signers::Signer::address(&signer)))
    }

    async fn sign_transaction(
        &self,
        wallet_index: u32,
        transaction: &TypedTransaction,
        chain_id: &ChainId,
    ) -> Result<Signature, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;

        let signature = match transaction {
            TypedTransaction::Legacy(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip2930(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip1559(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip4844(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
            TypedTransaction::Eip7702(tx) => {
                let mut tx = tx.clone();
                TxSigner::sign_transaction(&signer, &mut tx).await?
            }
        };

        Ok(signature)
    }

    async fn sign_text(
        &self,
        wallet_index: u32,
        text: &str,
        chain_id: &ChainId,
    ) -> Result<Signature, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;
        let signature = signer.sign_message(text.as_bytes()).await?;
        Ok(signature)
    }

    async fn sign_typed_data(
        &self,
        wallet_index: u32,
        typed_data: &TypedData,
        chain_id: &ChainId,
    ) -> Result<Signature, WalletError> {
        let signer = self.get_or_initialize_signer(wallet_index, chain_id).await?;

        let hash = typed_data.eip712_signing_hash()?;
        let signature = signer.sign_hash(&hash).await?;
        Ok(signature)
    }

    fn supports_blobs(&self) -> bool {
        true
    }
}
