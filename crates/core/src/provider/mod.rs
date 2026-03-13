use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

use crate::wallet::get_mnemonic_from_signing_key;
#[cfg(feature = "aws")]
use crate::wallet::AwsKmsWalletManager;
#[cfg(feature = "fireblocks")]
use crate::wallet::FireblocksWalletManager;
use crate::wallet::MnemonicWalletManager;
#[cfg(feature = "pkcs11")]
use crate::wallet::Pkcs11WalletManager;
#[cfg(feature = "privy")]
use crate::wallet::PrivyWalletManager;
#[cfg(feature = "turnkey")]
use crate::wallet::TurnkeyWalletManager;
use crate::{gas::get_gas_estimator, network::ChainId, SetupConfig, SigningProvider, WalletError};

mod evm_provider;
mod layer_extensions;

use self::evm_provider::EvmProviderNewError;
use crate::gas::GasEstimatorError;
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

        let has_main_signing_provider = signing_key.has_main_signing_provider();

        let gas_estimator = get_gas_estimator(&config.provider_urls, setup_config, config).await?;

        // If we only have private keys and no main signing provider, use private key manager only
        if let Some(private_keys) = &private_key_strings {
            if !has_main_signing_provider {
                let provider =
                    EvmProvider::new_with_private_keys(config, private_keys.clone(), gas_estimator)
                        .await?;

                providers.push(provider);
                continue;
            }
        }

        #[allow(unused_mut)]
        let mut provider: Option<EvmProvider> = None;

        #[cfg(feature = "privy")]
        if provider.is_none() {
            if let Some(privy) = &signing_key.privy {
                provider = Some(if private_key_strings.is_some() {
                    let privy_manager = Arc::new(
                        PrivyWalletManager::new(privy.app_id.clone(), privy.app_secret.clone())
                            .await?,
                    );
                    EvmProvider::new_with_composite(
                        config,
                        privy_manager,
                        private_key_strings.clone(),
                        gas_estimator.clone(),
                    )
                    .await?
                } else {
                    EvmProvider::new_with_privy(
                        config,
                        privy.app_id.clone(),
                        privy.app_secret.clone(),
                        gas_estimator.clone(),
                    )
                    .await?
                });
            }
        }

        #[cfg(feature = "aws")]
        if provider.is_none() {
            if let Some(aws_kms) = &signing_key.aws_kms {
                provider = Some(if private_key_strings.is_some() {
                    let aws_manager = Arc::new(AwsKmsWalletManager::new(aws_kms.clone()));
                    EvmProvider::new_with_composite(
                        config,
                        aws_manager,
                        private_key_strings.clone(),
                        gas_estimator.clone(),
                    )
                    .await?
                } else {
                    EvmProvider::new_with_aws_kms(config, aws_kms.clone(), gas_estimator.clone())
                        .await?
                });
            }
        }

        #[cfg(feature = "turnkey")]
        if provider.is_none() {
            if let Some(turnkey) = &signing_key.turnkey {
                provider = Some(if private_key_strings.is_some() {
                    let turnkey_manager =
                        Arc::new(TurnkeyWalletManager::new(turnkey.clone()).await?);
                    EvmProvider::new_with_composite(
                        config,
                        turnkey_manager,
                        private_key_strings.clone(),
                        gas_estimator.clone(),
                    )
                    .await?
                } else {
                    EvmProvider::new_with_turnkey(config, turnkey.clone(), gas_estimator.clone())
                        .await?
                });
            }
        }

        #[cfg(feature = "pkcs11")]
        if provider.is_none() {
            if let Some(pkcs11) = &signing_key.pkcs11 {
                provider = Some(if private_key_strings.is_some() {
                    let pkcs11_manager = Arc::new(Pkcs11WalletManager::new(pkcs11.clone())?);
                    EvmProvider::new_with_composite(
                        config,
                        pkcs11_manager,
                        private_key_strings.clone(),
                        gas_estimator.clone(),
                    )
                    .await?
                } else {
                    EvmProvider::new_with_pkcs11(config, pkcs11.clone(), gas_estimator.clone())
                        .await?
                });
            }
        }

        #[cfg(feature = "fireblocks")]
        if provider.is_none() {
            if let Some(fireblocks) = &signing_key.fireblocks {
                provider = Some(if private_key_strings.is_some() {
                    let fireblocks_manager =
                        Arc::new(FireblocksWalletManager::new(fireblocks.clone()).await?);
                    EvmProvider::new_with_composite(
                        config,
                        fireblocks_manager,
                        private_key_strings.clone(),
                        gas_estimator.clone(),
                    )
                    .await?
                } else {
                    EvmProvider::new_with_fireblocks(
                        config,
                        fireblocks.clone(),
                        gas_estimator.clone(),
                    )
                    .await?
                });
            }
        }

        // Fallback to mnemonic-based signing (raw, aws_secret_manager, gcp_secret_manager)
        let provider = if let Some(p) = provider {
            p
        } else {
            let mnemonic = get_mnemonic_from_signing_key(project_path, signing_key).await?;

            if private_key_strings.is_some() {
                let mnemonic_manager = Arc::new(MnemonicWalletManager::new(&mnemonic));
                EvmProvider::new_with_composite(
                    config,
                    mnemonic_manager,
                    private_key_strings,
                    gas_estimator,
                )
                .await?
            } else {
                EvmProvider::new_with_mnemonic(config, &mnemonic, gas_estimator).await?
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
