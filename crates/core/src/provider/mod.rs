use std::path::PathBuf;

use thiserror::Error;

use crate::{
    gas::fee_estimator::base::get_gas_estimator, network::types::ChainId, SetupConfig, SigningKey,
};

mod evm_provider;

use self::evm_provider::EvmProviderNewError;
use crate::wallet::get_mnemonic_from_signing_key;
pub use evm_provider::{
    create_retry_client, EvmProvider, RelayerProvider, RetryClientError, SendTransactionError,
};

#[derive(Error, Debug)]
pub enum LoadProvidersError {
    #[error("No signing_key in the yaml")]
    NoSigningKey,

    #[error("Signing key failed to load: {0}")]
    SigningKeyError(String),

    #[error("Providers are required in the yaml")]
    ProvidersRequired,

    #[error("{0}")]
    EvmProviderNewError(#[from] EvmProviderNewError),

    #[error("Gas estimator error {0}")]
    GasEstimatorError(String),
}

pub async fn load_providers(
    project_path: &PathBuf,
    setup_config: &SetupConfig,
) -> Result<Vec<EvmProvider>, LoadProvidersError> {
    let mut providers = Vec::new();

    for config in &setup_config.networks {
        if config.signing_key.is_none() && setup_config.signing_key.is_none() {
            return Err(LoadProvidersError::NoSigningKey);
        }

        if config.provider_urls.is_empty() {
            return Err(LoadProvidersError::ProvidersRequired);
        }

        let signing_key: &SigningKey = if let Some(ref signing_key) = config.signing_key {
            signing_key
        } else if let Some(ref signing_key) = setup_config.signing_key {
            signing_key
        } else {
            return Err(LoadProvidersError::NoSigningKey);
        };

        // Create the appropriate provider based on signing key type
        let provider = if let Some(privy) = &signing_key.privy {
            // Use Privy wallet manager
            EvmProvider::new_with_privy(
                &config.provider_urls,
                &config.name,
                privy.app_id.clone(),
                privy.app_secret.clone(),
                get_gas_estimator(&config.provider_urls, setup_config, config)
                    .await
                    .map_err(|e| LoadProvidersError::GasEstimatorError(e.to_string()))?,
            )
            .await?
        } else {
            let mnemonic =
                get_mnemonic_from_signing_key(project_path, &setup_config.name, signing_key)
                    .await
                    .map_err(|e| LoadProvidersError::SigningKeyError(e.to_string()))?;

            EvmProvider::new_with_mnemonic(
                &config.provider_urls,
                &config.name,
                &mnemonic,
                get_gas_estimator(&config.provider_urls, setup_config, config)
                    .await
                    .map_err(|e| LoadProvidersError::GasEstimatorError(e.to_string()))?,
            )
            .await?
        };

        providers.push(provider);
    }

    Ok(providers)
}

pub async fn find_provider_for_chain_id<'a>(
    providers: &'a Vec<EvmProvider>,
    chain_id: &ChainId,
) -> Option<&'a EvmProvider> {
    for provider in providers {
        if &provider.chain_id == chain_id {
            return Some(provider);
        }
    }

    None
}

pub fn chain_enabled(providers: &Vec<EvmProvider>, chain_id: &ChainId) -> bool {
    for provider in providers {
        if &provider.chain_id == chain_id {
            return true;
        }
    }

    false
}
