use std::{env, fs::File, io::Read, path::PathBuf};

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};

use crate::{
    gas::{
        fee_estimator::{
            custom::CustomGasFeeEstimator, infura::InfuraGasProviderSetupConfig,
            tenderly::TenderlyGasProviderSetupConfig,
        },
        types::{deserialize_gas_provider, GasProvider},
    },
    shared::common_types::EvmAddress,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AwsSigningKey {
    pub secret_name: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session_token: Option<String>,
    pub region: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawSigningKey {
    pub mnemonic: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeystoreSigningKey {
    pub path: String,
    pub account_name: String,
    // pub dangerous_define_raw_password:
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SigningKey {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub keystore: Option<KeystoreSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub raw: Option<RawSigningKey>,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aws_secret_manager: Option<AwsSigningKey>,
}

impl SigningKey {
    pub fn from_keystore(keystore: KeystoreSigningKey) -> Self {
        Self { keystore: Some(keystore), raw: None, aws_secret_manager: None }
    }
}

impl SigningKey {
    pub fn validate(&self) -> Result<(), String> {
        if self.raw.is_none() && self.aws_secret_manager.is_none() && self.keystore.is_none() {
            return Err("Signing key is not set".to_string());
        }

        if self.raw.is_some() && self.aws_secret_manager.is_some() {
            return Err("Signing key can not be both raw and aws secret manager".to_string());
        }

        if self.raw.is_some() && self.keystore.is_some() {
            return Err("Signing key can not be both raw and keystore".to_string());
        }

        if self.aws_secret_manager.is_some() && self.keystore.is_some() {
            return Err("Signing key can not be both aws secret manager and keystore".to_string());
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GasProviders {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub infura: Option<InfuraGasProviderSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tenderly: Option<TenderlyGasProviderSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub custom: Option<CustomGasFeeEstimator>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetupConfig {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signing_key: Option<SigningKey>,
    pub admins: Vec<EvmAddress>,
    pub networks: Vec<NetworkSetupConfig>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gas_providers: Option<GasProviders>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub allowed_origins: Option<Vec<String>>,
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

pub fn read(file_path: &PathBuf) -> Result<SetupConfig, ReadYamlError> {
    let mut file = File::open(file_path).map_err(|_| ReadYamlError::CanNotFindYaml)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).map_err(|_| ReadYamlError::CanNotReadYaml)?;

    let substituted_contents = substitute_env_variables(&contents)?;

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
