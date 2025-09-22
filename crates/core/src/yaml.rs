use alloy::primitives::utils::{parse_units, ParseUnits};
use alloy::primitives::U256;
use alloy::providers::Provider;
use regex::{Captures, Regex};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{env, fmt, fs::File, io::Read, path::PathBuf};
use thiserror::Error;
use tracing::error;

use crate::gas::{
    deserialize_gas_provider, CustomGasFeeEstimator, GasProvider, InfuraGasProviderSetupConfig,
    TenderlyGasProviderSetupConfig,
};
use crate::network::ChainId;
use crate::{create_retry_client, rrelayer_error, shared::common_types::EvmAddress};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GcpSigningKey {
    pub secret_name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version: Option<String>,
    pub service_account_key_path: String,
    pub secret_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AwsSigningKey {
    pub secret_name: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session_token: Option<String>,
    pub region: String,
    pub secret_key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RawSigningKey {
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrivySigningKey {
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TurnkeySigningKey {
    pub api_public_key: String,
    pub api_private_key: String,
    pub organization_id: String,
    pub wallet_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AwsKmsSigningKey {
    /// AWS KMS key IDs mapped by wallet index
    /// Can be a single key ID string or an array of key IDs
    pub key_ids: KmsKeyIds,
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub access_key_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub secret_access_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum KmsKeyIds {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SigningKey {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub raw: Option<RawSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aws_secret_manager: Option<AwsSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gcp_secret_manager: Option<GcpSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub privy: Option<PrivySigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aws_kms: Option<AwsKmsSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub turnkey: Option<TurnkeySigningKey>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimitConfig {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub limits: Option<RateLimits>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub user_unlimited_overrides: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub global_limits: Option<GlobalRateLimits>,
    #[serde(default)]
    pub fallback_to_relayer: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimits {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub transactions_per_minute: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signing_operations_per_minute: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GlobalRateLimits {
    /// Maximum transactions per minute across all relayers combined
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_transactions_per_minute: Option<u64>,
    /// Maximum signing operations per minute across all relayers combined
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_signing_operations_per_minute: Option<u64>,
}

impl KmsKeyIds {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            KmsKeyIds::Single(key_id) => {
                if key_id.is_empty() {
                    return Err("Single KMS key ID cannot be empty".to_string());
                }
            }
            KmsKeyIds::Multiple(key_ids) => {
                if key_ids.is_empty() {
                    return Err("Multiple KMS key IDs cannot be empty".to_string());
                }
                for (index, key_id) in key_ids.iter().enumerate() {
                    if key_id.is_empty() {
                        return Err(format!("KMS key ID at index {} cannot be empty", index));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_key_for_index(&self, wallet_index: u32) -> Result<&str, String> {
        match self {
            KmsKeyIds::Single(key_id) => Ok(key_id),
            KmsKeyIds::Multiple(key_ids) => {
                let index = wallet_index as usize;
                if index >= key_ids.len() {
                    return Err(format!(
                        "Wallet index {} is out of bounds for {} KMS keys",
                        wallet_index,
                        key_ids.len()
                    ));
                }
                Ok(&key_ids[index])
            }
        }
    }
}

impl AwsKmsSigningKey {
    pub fn validate(&self) -> Result<(), String> {
        if self.region.is_empty() {
            return Err("AWS region cannot be empty".to_string());
        }
        self.key_ids.validate()
    }
}

impl TurnkeySigningKey {
    pub fn validate(&self) -> Result<(), String> {
        if self.api_public_key.is_empty() {
            return Err("Turnkey API public key cannot be empty".to_string());
        }
        if self.api_private_key.is_empty() {
            return Err("Turnkey API private key cannot be empty".to_string());
        }
        if self.organization_id.is_empty() {
            return Err("Turnkey organization ID cannot be empty".to_string());
        }
        if self.wallet_id.is_empty() {
            return Err("Turnkey wallet ID cannot be empty".to_string());
        }
        Ok(())
    }
}

impl SigningKey {
    pub fn from_raw(raw: RawSigningKey) -> Self {
        Self {
            raw: Some(raw),
            aws_secret_manager: None,
            gcp_secret_manager: None,
            privy: None,
            aws_kms: None,
            turnkey: None,
        }
    }

    pub fn from_aws_kms(aws_kms: AwsKmsSigningKey) -> Self {
        Self {
            raw: None,
            aws_secret_manager: None,
            gcp_secret_manager: None,
            privy: None,
            aws_kms: Some(aws_kms),
            turnkey: None,
        }
    }

    pub fn from_turnkey(turnkey: TurnkeySigningKey) -> Self {
        Self {
            raw: None,
            aws_secret_manager: None,
            gcp_secret_manager: None,
            privy: None,
            aws_kms: None,
            turnkey: Some(turnkey),
        }
    }

    pub fn from_aws_kms_single(key_id: String, region: String) -> Self {
        let aws_kms = AwsKmsSigningKey {
            key_ids: KmsKeyIds::Single(key_id),
            region,
            access_key_id: None,
            secret_access_key: None,
            session_token: None,
        };
        Self::from_aws_kms(aws_kms)
    }

    pub fn from_aws_kms_multiple(key_ids: Vec<String>, region: String) -> Self {
        let aws_kms = AwsKmsSigningKey {
            key_ids: KmsKeyIds::Multiple(key_ids),
            region,
            access_key_id: None,
            secret_access_key: None,
            session_token: None,
        };
        Self::from_aws_kms(aws_kms)
    }
}

impl SigningKey {
    pub fn validate(&self) -> Result<(), String> {
        let configured_methods = [
            self.raw.is_some(),
            self.aws_secret_manager.is_some(),
            self.gcp_secret_manager.is_some(),
            self.privy.is_some(),
            self.aws_kms.is_some(),
            self.turnkey.is_some(),
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        match configured_methods {
            0 => Err("Signing key is not set".to_string()),
            1 => Ok(()),
            _ => Err("Only one signing key method can be configured at a time".to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkSetupConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signing_key: Option<SigningKey>,
    pub provider_urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub block_explorer_url: Option<String>,
    #[serde(
        deserialize_with = "deserialize_gas_provider",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub gas_provider: Option<GasProvider>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub automatic_top_up: Option<AutomaticTopUpConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub confirmations: Option<u64>,
}

impl NetworkSetupConfig {
    pub async fn get_chain_id(&self) -> Result<ChainId, String> {
        let provider_url = self.provider_urls[0].clone();

        let provider = create_retry_client(&provider_url)
            .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;
        let chain_id = provider
            .get_chain_id()
            .await
            .map_err(|e| format!("RPC provider is not valid as cannot get chain ID: {}", e))?;

        Ok(ChainId::new(chain_id))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GasProviders {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub infura: Option<InfuraGasProviderSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tenderly: Option<TenderlyGasProviderSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub custom: Option<CustomGasFeeEstimator>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiConfig {
    pub port: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub allowed_origins: Option<Vec<String>>,
    pub authentication_username: String,
    pub authentication_password: String,
}

#[derive(Debug, Clone)]
pub enum TopUpTargetAddresses {
    All,
    List(Vec<EvmAddress>),
}

impl Serialize for TopUpTargetAddresses {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            TopUpTargetAddresses::All => serializer.serialize_str("*"),
            TopUpTargetAddresses::List(addresses) => addresses.serialize(serializer),
        }
    }
}

struct ForAddressesTypeVisitor;

impl<'de> Visitor<'de> for ForAddressesTypeVisitor {
    type Value = TopUpTargetAddresses;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("either '*' for all addresses or a list of addresses")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value == "*" {
            Ok(TopUpTargetAddresses::All)
        } else {
            Err(de::Error::invalid_value(de::Unexpected::Str(value), &"'*' for all addresses"))
        }
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let addresses = Vec::<EvmAddress>::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
        Ok(TopUpTargetAddresses::List(addresses))
    }
}

impl<'de> Deserialize<'de> for TopUpTargetAddresses {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ForAddressesTypeVisitor)
    }
}

#[derive(Debug, Clone)]
pub struct NativeTokenConfig {
    pub enabled: bool,
    pub min_balance: U256,
    pub top_up_amount: U256,
    pub decimals: u8,
}

impl Serialize for NativeTokenConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("NativeTokenConfig", 4)?;
        state.serialize_field("enabled", &self.enabled)?;
        state.serialize_field(
            "min_balance",
            &serialize_amount_with_decimals(&self.min_balance, self.decimals),
        )?;
        state.serialize_field(
            "top_up_amount",
            &serialize_amount_with_decimals(&self.top_up_amount, self.decimals),
        )?;
        state.serialize_field("decimals", &self.decimals)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for NativeTokenConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NativeTokenConfigHelper {
            enabled: bool,
            min_balance: String,
            top_up_amount: String,
            #[serde(default = "default_decimals")]
            decimals: u8,
        }

        let helper = NativeTokenConfigHelper::deserialize(deserializer)?;

        let min_balance = parse_units(&helper.min_balance, helper.decimals)
            .unwrap_or(ParseUnits::U256(U256::ZERO))
            .into();
        let top_up_amount = parse_units(&helper.top_up_amount, helper.decimals)
            .unwrap_or(ParseUnits::U256(U256::ZERO))
            .into();

        Ok(NativeTokenConfig {
            enabled: helper.enabled,
            min_balance,
            top_up_amount,
            decimals: helper.decimals,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Erc20TokenConfig {
    pub address: EvmAddress,
    pub min_balance: U256,
    pub top_up_amount: U256,
    pub decimals: u8,
}

impl Serialize for Erc20TokenConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Erc20TokenConfig", 4)?;
        state.serialize_field("address", &self.address)?;
        state.serialize_field(
            "min_balance",
            &serialize_amount_with_decimals(&self.min_balance, self.decimals),
        )?;
        state.serialize_field(
            "top_up_amount",
            &serialize_amount_with_decimals(&self.top_up_amount, self.decimals),
        )?;
        state.serialize_field("decimals", &self.decimals)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Erc20TokenConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Erc20TokenConfigHelper {
            address: EvmAddress,
            min_balance: String,
            top_up_amount: String,
            #[serde(default = "default_decimals")]
            decimals: u8,
        }

        let helper = Erc20TokenConfigHelper::deserialize(deserializer)?;

        let min_balance = parse_units(&helper.min_balance, helper.decimals)
            .unwrap_or(ParseUnits::U256(U256::ZERO))
            .into();
        let top_up_amount = parse_units(&helper.top_up_amount, helper.decimals)
            .unwrap_or(ParseUnits::U256(U256::ZERO))
            .into();

        Ok(Erc20TokenConfig {
            address: helper.address,
            min_balance,
            top_up_amount,
            decimals: helper.decimals,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AutomaticTopUpConfig {
    pub from_address: EvmAddress,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub safe: Option<EvmAddress>,
    pub targets: TopUpTargetAddresses,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub native: Option<NativeTokenConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub erc20_tokens: Option<Vec<Erc20TokenConfig>>,
}

/// Default decimals value (18 for ETH-like tokens)
fn default_decimals() -> u8 {
    18
}

/// Serialize an amount with custom decimal precision
fn serialize_amount_with_decimals(amount: &U256, decimals: u8) -> String {
    let divisor = U256::from(10u64.pow(decimals as u32));
    let whole_part = amount / divisor;
    let remainder = amount % divisor;

    if remainder.is_zero() {
        format!("{}", whole_part)
    } else {
        let decimal_str = format!("{:0width$}", remainder, width = decimals as usize);
        let decimal_trimmed = decimal_str.trim_end_matches('0');
        format!("{}.{}", whole_part, decimal_trimmed)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebhookConfig {
    pub endpoint: String,
    pub shared_secret: String,
    pub networks: Vec<String>,
    pub max_retries: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebhookConfigAdvanced {
    pub endpoint: String,
    pub shared_secret: String,
    pub networks: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub timeout_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub retry_attempts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SafeProxyConfig {
    pub address: EvmAddress,
    pub relayers: Vec<EvmAddress>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetupConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signing_key: Option<SigningKey>,
    pub networks: Vec<NetworkSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gas_providers: Option<GasProviders>,
    pub api_config: ApiConfig,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub webhooks: Option<Vec<WebhookConfig>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub safe_proxy: Option<Vec<SafeProxyConfig>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rate_limits: Option<RateLimitConfig>,
}

/// Substitutes environment variables in YAML content.
fn substitute_env_variables(contents: &str) -> Result<String, regex::Error> {
    let re = Regex::new(r"\$\{([^}]+)\}")?;
    let result = re.replace_all(contents, |caps: &Captures| {
        let var_name = &caps[1];
        match env::var(var_name) {
            Ok(val) => val,
            Err(_) => {
                rrelayer_error!("Environment variable {} not found", var_name);
                panic!("Environment variable {} not found", var_name)
            }
        }
    });
    Ok(result.into_owned())
}

#[derive(Error, Debug)]
pub enum ReadYamlError {
    #[error("Can not find yaml")]
    CanNotFindYaml,

    #[error("Can not read yaml")]
    CanNotReadYaml,

    #[error("Setup config is invalid yaml and does not match the struct - {0}")]
    SetupConfigInvalidYaml(String),

    #[error("Environment variable {} not found", {0})]
    EnvironmentVariableNotFound(#[from] regex::Error),

    #[error("No networks enabled in the yaml")]
    NoNetworksEnabled,

    #[error("Signing key yaml bad format: {0}")]
    SigningKeyYamlError(String),

    #[error("Network {0} provider urls not defined")]
    NetworkProviderUrlsNotDefined(String),
}

/// Reads and parses the RRelayer configuration YAML file.
pub fn read(file_path: &PathBuf, raw_yaml: bool) -> Result<SetupConfig, ReadYamlError> {
    let mut file = File::open(file_path).map_err(|_| ReadYamlError::CanNotFindYaml)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|_| ReadYamlError::CanNotReadYaml)?;

    let substituted_contents =
        if raw_yaml { contents } else { substitute_env_variables(&contents)? };

    let config: SetupConfig = serde_yaml::from_str(&substituted_contents)
        .map_err(|e| ReadYamlError::SetupConfigInvalidYaml(e.to_string()))?;

    if config.networks.is_empty() {
        return Err(ReadYamlError::NoNetworksEnabled);
    }

    for network in &config.networks {
        if network.provider_urls.is_empty() {
            return Err(ReadYamlError::NetworkProviderUrlsNotDefined(network.name.clone()));
        }

        if let Some(signing_key) = &network.signing_key {
            signing_key.validate().map_err(ReadYamlError::SigningKeyYamlError)?;
        }
    }

    if let Some(signing_key) = &config.signing_key {
        signing_key.validate().map_err(ReadYamlError::SigningKeyYamlError)?;

        if let Some(aws_kms) = &signing_key.aws_kms {
            aws_kms.validate().map_err(ReadYamlError::SigningKeyYamlError)?;
        }

        if let Some(turnkey) = &signing_key.turnkey {
            turnkey.validate().map_err(ReadYamlError::SigningKeyYamlError)?;
        }
    }

    Ok(config)
}
