use crate::wallet::WalletError;
use crate::SigningKey;
use std::path::PathBuf;

mod aws_secret_manager;
use aws_secret_manager::get_aws_secret;
mod gcp_secret_manager;
use crate::keystore::{recover_mnemonic_from_keystore, KeyStorePasswordManager};
use gcp_secret_manager::get_gcp_secret;

pub mod keystore;

/// Retrieves a mnemonic phrase from various signing key sources.
///
/// This function supports multiple methods for retrieving mnemonic phrases:
/// - Raw mnemonic directly from configuration
/// - AWS Secrets Manager
/// - GCP Secret Manager  
/// - Local keystore files
///
/// The function tries each method in order and returns the first successful result.
///
/// # Arguments
/// * `project_path` - Path to the project directory for resolving relative paths
/// * `project_name` - Name of the project for keystore password management
/// * `signing_key` - Configuration specifying which method to use for mnemonic retrieval
///
/// # Returns
/// * `Ok(String)` - The retrieved mnemonic phrase
/// * `Err(WalletError)` - If no signing key method is configured or all methods fail
///
/// # Errors
/// Returns `WalletError::NoSigningKey` if no signing key source is configured.
/// Other errors depend on the specific method used (API errors, authentication errors, etc.)
pub async fn get_mnemonic_from_signing_key(
    project_path: &PathBuf,
    project_name: &str,
    signing_key: &SigningKey,
) -> Result<String, WalletError> {
    if let Some(raw) = &signing_key.raw {
        return Ok(raw.mnemonic.clone());
    }

    if let Some(aws_secret_manager) = &signing_key.aws_secret_manager {
        let result = get_aws_secret(aws_secret_manager).await?;
        return Ok(result);
    }

    if let Some(gcp_secret_manager) = &signing_key.gcp_secret_manager {
        let result = get_gcp_secret(project_path, gcp_secret_manager).await?;
        return Ok(result);
    }

    if let Some(keystore) = &signing_key.keystore {
        let password = if let Some(pwd) = keystore.dangerous_define_raw_password.clone() {
            pwd
        } else {
            KeyStorePasswordManager::new(project_name)
                .map_err(|e| WalletError::ConfigurationError { message: format!("Failed to create password manager: {}", e) })?
                .load(&keystore.name)
                .map_err(|_| WalletError::AuthenticationError { message: "Server is not authenticated to use the keystores rrelayer_signing_key please login on the server".to_string() })?
        };
        let keystore_path = project_path.join(&keystore.path);
        let result = recover_mnemonic_from_keystore(&keystore_path, &password)?;
        return Ok(result);
    }

    Err(WalletError::NoSigningKey)
}
