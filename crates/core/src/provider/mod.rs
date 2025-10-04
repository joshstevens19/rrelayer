use std::path::Path;

use thiserror::Error;

use crate::{gas::get_gas_estimator, network::ChainId, SetupConfig, SigningProvider, WalletError};

mod evm_provider;
mod layer_extensions;

use self::evm_provider::EvmProviderNewError;
use crate::gas::GasEstimatorError;
use crate::wallet::get_mnemonic_from_signing_key;
pub use evm_provider::{
    create_retry_client, EvmProvider, RelayerProvider, RetryClientError, SendTransactionError,
};

#[derive(Error, Debug)]
pub enum LoadProvidersError {
    #[error("No signing_key in the yaml")]
    NoSigningKey,

    #[error("Signing key failed to load: {0}")]
    SigningKeyError(#[from] WalletError),

    #[error("Providers are required in the yaml")]
    ProvidersRequired,

    #[error("{0}")]
    EvmProviderNewError(#[from] EvmProviderNewError),

    #[error(transparent)]
    GasEstimatorError(#[from] GasEstimatorError),
}

/// Loads and initializes EVM providers for all configured networks.
pub async fn load_providers(
    project_path: &Path,
    setup_config: &SetupConfig,
) -> Result<Vec<EvmProvider>, LoadProvidersError> {
    let mut providers = Vec::new();

    for config in &setup_config.networks {
        if config.signing_provider.is_none() && setup_config.signing_provider.is_none() {
            return Err(LoadProvidersError::NoSigningKey);
        }

        if config.provider_urls.is_empty() {
            return Err(LoadProvidersError::ProvidersRequired);
        }

        let signing_key: &SigningProvider = if let Some(ref signing_key) = config.signing_provider {
            signing_key
        } else if let Some(ref signing_key) = setup_config.signing_provider {
            signing_key
        } else {
            return Err(LoadProvidersError::NoSigningKey);
        };

        // Extract private keys if configured
        let private_key_strings: Option<Vec<String>> = signing_key
            .private_keys
            .as_ref()
            .map(|private_keys| private_keys.iter().map(|pk| pk.raw.clone()).collect());

        // Check if we have a main signing provider (non-private-key)
        let has_main_signing_provider = signing_key.privy.is_some()
            || signing_key.aws_kms.is_some()
            || signing_key.turnkey.is_some()
            || signing_key.pkcs11.is_some()
            || signing_key.raw.is_some();

        // If we only have private keys and no main signing provider, use private key manager only
        if private_key_strings.is_some() && !has_main_signing_provider {
            let provider = EvmProvider::new_with_private_keys(
                config,
                private_key_strings.unwrap(),
                get_gas_estimator(&config.provider_urls, setup_config, config).await?,
            )
            .await?;

            providers.push(provider);
            continue;
        }

        let provider = if let Some(privy) = &signing_key.privy {
            if private_key_strings.is_some() {
                // Use composite manager with privy + private keys
                let privy_manager = std::sync::Arc::new(
                    crate::wallet::PrivyWalletManager::new(
                        privy.app_id.clone(),
                        privy.app_secret.clone(),
                    )
                    .await?,
                );
                EvmProvider::new_with_composite(
                    config,
                    privy_manager,
                    private_key_strings,
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            } else {
                EvmProvider::new_with_privy(
                    config,
                    privy.app_id.clone(),
                    privy.app_secret.clone(),
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            }
        } else if let Some(aws_kms) = &signing_key.aws_kms {
            if private_key_strings.is_some() {
                // Use composite manager with aws_kms + private keys
                let aws_manager =
                    std::sync::Arc::new(crate::wallet::AwsKmsWalletManager::new(aws_kms.clone()));
                EvmProvider::new_with_composite(
                    config,
                    aws_manager,
                    private_key_strings,
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            } else {
                EvmProvider::new_with_aws_kms(
                    config,
                    aws_kms.clone(),
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            }
        } else if let Some(turnkey) = &signing_key.turnkey {
            if private_key_strings.is_some() {
                // Use composite manager with turnkey + private keys
                let turnkey_manager = std::sync::Arc::new(
                    crate::wallet::TurnkeyWalletManager::new(turnkey.clone()).await?,
                );
                EvmProvider::new_with_composite(
                    config,
                    turnkey_manager,
                    private_key_strings,
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            } else {
                EvmProvider::new_with_turnkey(
                    config,
                    turnkey.clone(),
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            }
        } else if let Some(pkcs11) = &signing_key.pkcs11 {
            if private_key_strings.is_some() {
                let pkcs11_manager =
                    std::sync::Arc::new(crate::wallet::Pkcs11WalletManager::new(pkcs11.clone())?);
                EvmProvider::new_with_composite(
                    config,
                    pkcs11_manager,
                    private_key_strings,
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            } else {
                EvmProvider::new_with_pkcs11(
                    config,
                    pkcs11.clone(),
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            }
        } else {
            let mnemonic = get_mnemonic_from_signing_key(project_path, signing_key).await?;

            if private_key_strings.is_some() {
                // Use composite manager with mnemonic + private keys
                let mnemonic_manager =
                    std::sync::Arc::new(crate::wallet::MnemonicWalletManager::new(&mnemonic));
                EvmProvider::new_with_composite(
                    config,
                    mnemonic_manager,
                    private_key_strings,
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            } else {
                EvmProvider::new_with_mnemonic(
                    config,
                    &mnemonic,
                    get_gas_estimator(&config.provider_urls, setup_config, config).await?,
                )
                .await?
            }
        };

        providers.push(provider);
    }

    Ok(providers)
}

/// Finds an EVM provider for a specific chain ID.
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
pub fn chain_enabled(providers: &Vec<EvmProvider>, chain_id: &ChainId) -> bool {
    for provider in providers {
        if &provider.chain_id == chain_id {
            return true;
        }
    }

    false
}
