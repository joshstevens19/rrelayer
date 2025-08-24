use alloy::primitives::utils::{parse_units, ParseUnits};
use alloy::primitives::U256;
use alloy::providers::Provider;
use regex::{Captures, Regex};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{env, fmt, fs::File, io::Read, path::PathBuf};
use thiserror::Error;
use tracing::{error, info};

use crate::network::types::ChainId;
use crate::{
    create_retry_client,
    gas::{
        fee_estimator::{
            custom::CustomGasFeeEstimator, infura::InfuraGasProviderSetupConfig,
            tenderly::TenderlyGasProviderSetupConfig,
        },
        types::{deserialize_gas_provider, GasProvider},
    },
    shared::common_types::EvmAddress,
};

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
pub struct KeystoreSigningKey {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub dangerous_define_raw_password: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrivySigningKey {
    pub app_id: String,
    pub app_secret: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SigningKey {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub keystore: Option<KeystoreSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub raw: Option<RawSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aws_secret_manager: Option<AwsSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gcp_secret_manager: Option<GcpSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub privy: Option<PrivySigningKey>,
}

impl SigningKey {
    pub fn from_keystore(keystore: KeystoreSigningKey) -> Self {
        Self {
            keystore: Some(keystore),
            raw: None,
            aws_secret_manager: None,
            gcp_secret_manager: None,
            privy: None,
        }
    }
}

impl SigningKey {
    pub fn validate(&self) -> Result<(), String> {
        let configured_methods = [
            self.raw.is_some(),
            self.aws_secret_manager.is_some(),
            self.gcp_secret_manager.is_some(),
            self.keystore.is_some(),
            self.privy.is_some(),
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
#[serde(untagged)]
pub enum AdminIdentifier {
    EvmAddress(EvmAddress),
    Name(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiConfig {
    pub port: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub allowed_origins: Option<Vec<String>>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NativeTokenConfig {
    pub enabled: bool,
    #[serde(deserialize_with = "deserialize_eth_amount", serialize_with = "serialize_eth_amount")]
    pub min_balance: U256,
    #[serde(deserialize_with = "deserialize_eth_amount", serialize_with = "serialize_eth_amount")]
    pub top_up_amount: U256,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Erc20TokenConfig {
    pub address: EvmAddress,
    #[serde(deserialize_with = "deserialize_token_amount", serialize_with = "serialize_token_amount")]
    pub min_balance: U256,
    #[serde(deserialize_with = "deserialize_token_amount", serialize_with = "serialize_token_amount")]
    pub top_up_amount: U256,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AutomaticTopUpConfig {
    pub from_address: EvmAddress,
    pub targets: TopUpTargetAddresses,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub native: Option<NativeTokenConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub erc20_tokens: Option<Vec<Erc20TokenConfig>>,
}

fn deserialize_eth_amount<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let result: U256 = parse_units(&s, 18).unwrap_or(ParseUnits::U256(U256::ZERO)).into();
    Ok(result)
}

fn serialize_eth_amount<S>(amount: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // Convert back to ETH string representation
    let eth_divisor = U256::from(10u64.pow(18));
    let whole_eth = amount / eth_divisor;
    let remainder = amount % eth_divisor;

    let eth_string = if remainder.is_zero() {
        format!("{}", whole_eth)
    } else {
        let decimal_str = format!("{:018}", remainder);
        let decimal_trimmed = decimal_str.trim_end_matches('0');
        format!("{}.{}", whole_eth, decimal_trimmed)
    };

    serializer.serialize_str(&eth_string)
}

// For ERC-20 tokens, we'll use 18 decimals as default but this can be extended
// to query the token contract for actual decimals in the future
fn deserialize_token_amount<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    // For now, assume 18 decimals for ERC-20 tokens (same as ETH)
    // This can be enhanced to support different token decimals
    let result: U256 = parse_units(&s, 18)
        .unwrap_or(ParseUnits::U256(U256::ZERO))
        .into();
    Ok(result)
}

fn serialize_token_amount<S>(amount: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // For now, use same logic as ETH (18 decimals)
    // This can be enhanced to support different token decimals
    serialize_eth_amount(amount, serializer)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetupConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signing_key: Option<SigningKey>,
    pub admins: Vec<AdminIdentifier>,
    pub networks: Vec<NetworkSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gas_providers: Option<GasProviders>,
    pub api_config: ApiConfig,
}

fn substitute_env_variables(contents: &str) -> Result<String, regex::Error> {
    let re = Regex::new(r"\$\{([^}]+)\}")?;
    let result = re.replace_all(contents, |caps: &Captures| {
        let var_name = &caps[1];
        match env::var(var_name) {
            Ok(val) => val,
            Err(_) => {
                error!("Environment variable {} not found", var_name);
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
    }

    Ok(config)
}
