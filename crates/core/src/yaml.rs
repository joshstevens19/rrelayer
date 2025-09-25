use alloy::primitives::utils::{parse_units, ParseUnits};
use alloy::primitives::U256;
use alloy::providers::Provider;
use regex::{Captures, Regex};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fs::Permissions;
use std::{env, fmt, fs::File, io::Read, path::PathBuf};
use thiserror::Error;
use tracing::error;

use crate::gas::{
    deserialize_gas_provider, CustomGasFeeEstimator, GasProvider, InfuraGasProviderSetupConfig,
    TenderlyGasProviderSetupConfig,
};
use crate::network::{ChainId, Network};
use crate::{create_retry_client, rrelayer_error, shared::common_types::EvmAddress};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GcpSecretManagerProviderConfig {
    pub id: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub version: Option<String>,
    pub service_account_key_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AwsSecretManagerProviderConfig {
    pub id: String,
    pub key: String,
    pub region: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AwsKmsSigningProviderConfig {
    pub region: String,
    pub danger_override_alias: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RawSigningProviderConfig {
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrivySigningProviderConfig {
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TurnkeySigningProviderConfig {
    pub api_public_key: String,
    pub api_private_key: String,
    pub organization_id: String,
    pub wallet_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SigningProvider {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub raw: Option<RawSigningProviderConfig>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aws_secret_manager: Option<AwsSecretManagerProviderConfig>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gcp_secret_manager: Option<GcpSecretManagerProviderConfig>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub privy: Option<PrivySigningProviderConfig>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aws_kms: Option<AwsKmsSigningProviderConfig>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub turnkey: Option<TurnkeySigningProviderConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimitWithInterval {
    pub interval: String,
    pub transactions: u64,
    pub signing_operations: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserRateLimitConfig {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub per_relayer: Option<RateLimitWithInterval>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub global: Option<RateLimitWithInterval>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RateLimitConfig {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub user_limits: Option<UserRateLimitConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relayer_limits: Option<RateLimitWithInterval>,
    #[serde(default)]
    pub fallback_to_relayer: bool,
}

impl AwsKmsSigningProviderConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.region.is_empty() {
            return Err("AWS KMS region cannot be empty".to_string());
        }
        Ok(())
    }
}

impl TurnkeySigningProviderConfig {
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

impl SigningProvider {
    pub fn from_raw(raw: RawSigningProviderConfig) -> Self {
        Self {
            raw: Some(raw),
            aws_secret_manager: None,
            gcp_secret_manager: None,
            privy: None,
            aws_kms: None,
            turnkey: None,
        }
    }

    pub fn from_aws_kms(aws_kms: AwsKmsSigningProviderConfig) -> Self {
        Self {
            raw: None,
            aws_secret_manager: None,
            gcp_secret_manager: None,
            privy: None,
            aws_kms: Some(aws_kms),
            turnkey: None,
        }
    }

    pub fn from_turnkey(turnkey: TurnkeySigningProviderConfig) -> Self {
        Self {
            raw: None,
            aws_secret_manager: None,
            gcp_secret_manager: None,
            privy: None,
            aws_kms: None,
            turnkey: Some(turnkey),
        }
    }
}

impl SigningProvider {
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
pub struct NetworkPermissionsConfig {
    pub relayers: AllOrAddresses,
    pub allowlist: Vec<EvmAddress>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub disable_native_transfer: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkSetupConfig {
    pub name: String,
    pub chain_id: ChainId,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signing_provider: Option<SigningProvider>,
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
    pub automatic_top_up: Option<NetworkAutomaticTopUpConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub permissions: Option<Vec<NetworkPermissionsConfig>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub confirmations: Option<u64>,
}

impl From<NetworkSetupConfig> for Network {
    fn from(value: NetworkSetupConfig) -> Self {
        Network { name: value.name, chain_id: value.chain_id, provider_urls: value.provider_urls }
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
pub enum AllOrAddresses {
    All,
    List(Vec<EvmAddress>),
}

impl AllOrAddresses {
    pub fn contains(&self, address: &EvmAddress) -> bool {
        match self {
            AllOrAddresses::All => true,
            AllOrAddresses::List(addresses) => addresses.contains(address),
        }
    }
}

impl Serialize for AllOrAddresses {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            AllOrAddresses::All => serializer.serialize_str("*"),
            AllOrAddresses::List(addresses) => addresses.serialize(serializer),
        }
    }
}

struct ForAddressesTypeVisitor;

impl<'de> Visitor<'de> for ForAddressesTypeVisitor {
    type Value = AllOrAddresses;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("either '*' for all addresses or a list of addresses")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value == "*" {
            Ok(AllOrAddresses::All)
        } else {
            Err(de::Error::invalid_value(de::Unexpected::Str(value), &"'*' for all addresses"))
        }
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let addresses = Vec::<EvmAddress>::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
        Ok(AllOrAddresses::List(addresses))
    }
}

impl<'de> Deserialize<'de> for AllOrAddresses {
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
pub struct NetworkAutomaticTopUpRelayer {
    pub address: EvmAddress,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub internal_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkAutomaticTopUpFrom {
    pub safe: Option<EvmAddress>,
    pub relayer: NetworkAutomaticTopUpRelayer,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkAutomaticTopUpConfig {
    pub from: NetworkAutomaticTopUpFrom,
    pub targets: AllOrAddresses,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub native: Option<NativeTokenConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub erc20_tokens: Option<Vec<Erc20TokenConfig>>,
}

impl NetworkAutomaticTopUpConfig {
    pub fn via_safe(&self) -> bool {
        self.from.safe.is_some()
    }
}

fn default_decimals() -> u8 {
    18
}

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
    pub chain_id: ChainId,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetupConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signing_provider: Option<SigningProvider>,
    pub networks: Vec<NetworkSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gas_providers: Option<GasProviders>,
    pub api_config: ApiConfig,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub webhooks: Option<Vec<WebhookConfig>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rate_limits: Option<RateLimitConfig>,
}

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

    #[error("Signing provider yaml bad format: {0}")]
    SigningProviderYamlError(String),

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

        if let Some(signing_key) = &network.signing_provider {
            signing_key.validate().map_err(ReadYamlError::SigningProviderYamlError)?;
        }
    }

    if let Some(signing_key) = &config.signing_provider {
        signing_key.validate().map_err(ReadYamlError::SigningProviderYamlError)?;

        if let Some(aws_kms) = &signing_key.aws_kms {
            aws_kms.validate().map_err(ReadYamlError::SigningProviderYamlError)?;
        }

        if let Some(turnkey) = &signing_key.turnkey {
            turnkey.validate().map_err(ReadYamlError::SigningProviderYamlError)?;
        }
    }

    Ok(config)
}
