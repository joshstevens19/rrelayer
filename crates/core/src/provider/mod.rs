use std::path::PathBuf;

use thiserror::Error;

use crate::{
    gas::fee_estimator::base::get_gas_estimator,
    network::types::ChainId,
    setup::yaml::{SetupConfig, SigningKey},
};

mod wallet_manager;
pub use wallet_manager::generate_seed_phrase;

mod evm_provider;
pub use evm_provider::{create_retry_client, EvmProvider, RelayerProvider, SendTransactionError};

use self::evm_provider::EvmProviderNewError;
use crate::setup::signing_key_providers::get_mnemonic_from_signing_key;

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

        let signing_key: &SigningKey = if config.signing_key.is_some() {
            config.signing_key.as_ref().unwrap()
        } else {
            setup_config.signing_key.as_ref().unwrap()
        };

        let result =
            get_mnemonic_from_signing_key(project_path, &setup_config.name, signing_key).await;

        match result {
            Ok(mnemonic) => {
                providers.push(
                    EvmProvider::new(
                        &config.provider_urls,
                        &config.name,
                        &mnemonic,
                        get_gas_estimator(setup_config, config),
                    )
                    .await?,
                );
            }
            Err(e) => return Err(LoadProvidersError::SigningKeyError(e.to_string())),
        }
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
