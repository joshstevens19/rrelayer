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

/// Loads and initializes EVM providers for all configured networks.
///
/// This function creates an EvmProvider instance for each network configuration,
/// setting up the appropriate signing mechanism (mnemonic or Privy) and gas estimator.
///
/// # Arguments
/// * `project_path` - Path to the project directory for loading signing keys
/// * `setup_config` - Configuration containing network settings and signing keys
///
/// # Returns
/// * `Ok(Vec<EvmProvider>)` - Vector of initialized EVM providers
/// * `Err(LoadProvidersError)` - Error if providers cannot be created
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
                &config,
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
                &config,
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

/// Finds an EVM provider for a specific chain ID.
///
/// Searches through the provided list of EVM providers to find one that matches
/// the specified chain ID.
///
/// # Arguments
/// * `providers` - List of available EVM providers
/// * `chain_id` - The chain ID to search for
///
/// # Returns
/// * `Some(&EvmProvider)` - Reference to the matching provider if found
/// * `None` - If no provider matches the chain ID
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

/// Checks if a specific chain ID is enabled in the provider configuration.
///
/// Determines whether any of the configured EVM providers support the specified chain ID.
///
/// # Arguments
/// * `providers` - List of available EVM providers
/// * `chain_id` - The chain ID to check for support
///
/// # Returns
/// * `true` - If at least one provider supports the chain ID
/// * `false` - If no providers support the chain ID
pub fn chain_enabled(providers: &Vec<EvmProvider>, chain_id: &ChainId) -> bool {
    for provider in providers {
        if &provider.chain_id == chain_id {
            return true;
        }
    }

    false
}
