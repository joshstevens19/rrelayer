use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
    time::{Duration, SystemTime},
};

use alloy::{
    primitives::{Address, B256},
    signers::{Signer, local::PrivateKeySigner},
};
use dialoguer::Password;
use rrelayer_core::{
    authentication::types::TokenPair,
    keystore::{KeyStorePasswordManager, KeystoreDecryptResult, PasswordError, decrypt_keystore},
};
use rrelayer_sdk::SDK;
use serde::{Deserialize, Serialize};

use crate::{commands::keystore::ProjectLocation, error::CliError};

/// Cached authentication token data with expiration tracking.
///
/// Stores authentication tokens securely on disk with automatic expiration
/// to prevent indefinite token reuse.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredTokenData {
    token_pair: TokenPair,
    address: String,
    expires_at: SystemTime,
}

/// Generates a secure file path for storing authentication tokens.
///
/// Creates a platform-appropriate configuration directory structure for token storage
/// with restricted permissions on Unix systems.
///
/// # Arguments
/// * `project_name` - Name of the rrelayer project
/// * `account` - Account identifier for the token
///
/// # Returns
/// * `PathBuf` - Secure path for token file storage
fn get_secure_token_path(project_name: &str, account: &str) -> PathBuf {
    let base_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rrelayer")
        .join(project_name)
        .join("auth_tokens");

    fs::create_dir_all(&base_dir).ok();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if base_dir.exists() {
            let permissions = fs::Permissions::from_mode(0o700); // rwx------
            fs::set_permissions(&base_dir, permissions).ok();
        }
    }

    base_dir.join(format!("{}.json", account))
}

/// Saves authentication token data to secure local cache.
///
/// Stores token pair, address, and expiration time to a JSON file with
/// restricted file permissions for security.
///
/// # Arguments
/// * `sdk` - SDK instance containing the token pair to save
/// * `project_name` - Name of the rrelayer project
/// * `account` - Account identifier for the token
/// * `address` - Ethereum address associated with the token
///
/// # Returns
/// * `Ok(())` - Token saved successfully
/// * `Err(CliError)` - Token save failed (no token available, file write error, etc.)
fn save_token_to_cache(
    sdk: &SDK,
    project_name: &str,
    account: &str,
    address: String,
) -> Result<(), CliError> {
    if let Some(token_pair) = &sdk.context.token_pair {
        let expires_at = SystemTime::now() + Duration::from_secs(300); // 5 mins

        let token_data = StoredTokenData { token_pair: token_pair.clone(), address, expires_at };

        let token_file = get_secure_token_path(project_name, account);

        let mut file = File::create(&token_file)?;
        let serialized = serde_json::to_string(&token_data)?;
        file.write_all(serialized.as_bytes())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&token_file, permissions)?;
        }

        Ok(())
    } else {
        Err(CliError::Internal("No token available to save".to_string()))
    }
}

/// Loads authentication token data from secure local cache.
///
/// Reads and validates cached token data, automatically removing expired tokens.
///
/// # Arguments
/// * `project_name` - Name of the rrelayer project
/// * `account` - Account identifier for the token
///
/// # Returns
/// * `Ok(Some(StoredTokenData))` - Valid token data found
/// * `Ok(None)` - No token found or token expired
/// * `Err(CliError)` - File read or deserialization error
fn load_token_from_cache(
    project_name: &str,
    account: &str,
) -> Result<Option<StoredTokenData>, CliError> {
    let token_file = get_secure_token_path(project_name, account);

    if !token_file.exists() {
        return Ok(None);
    }

    let mut file = File::open(&token_file)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let token_data: StoredTokenData = serde_json::from_str(&contents)?;

    let now = SystemTime::now();
    if token_data.expires_at <= now {
        fs::remove_file(token_file)?;
        return Ok(None);
    }

    Ok(Some(token_data))
}

/// Verifies that the rrelayer API server is running and accessible.
///
/// Performs a health check request to ensure the API server is available
/// before attempting authentication operations.
///
/// # Arguments
/// * `sdk` - SDK instance configured with API endpoint
///
/// # Returns
/// * `Ok(())` - API server is running and accessible
/// * `Err(CliError)` - API server is unreachable or not running
pub async fn check_api_running(sdk: &SDK) -> Result<(), CliError> {
    match sdk.health.check().await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error: API server is not running or is unreachable.");
            eprintln!("Please start the API server before continuing.");
            eprintln!("Details: {}", e);

            Err(CliError::Api(
                "The API server is not running. Please start it before continuing.".to_string(),
            ))
        }
    }
}

/// Handles the complete authentication flow for a relayer account.
///
/// Performs authentication using cached tokens when available, or prompts for
/// keystore password to authenticate with private key signature. Manages token
/// caching and refresh automatically.
///
/// # Arguments
/// * `sdk` - Mutable SDK instance to authenticate
/// * `account` - Account identifier to authenticate
/// * `project_location` - Project configuration for keystore access
///
/// # Returns
/// * `Ok(())` - Authentication successful
/// * `Err(CliError)` - Authentication failed (API unavailable, invalid credentials, etc.)
pub async fn handle_authenticate(
    sdk: &mut SDK,
    account: &str,
    project_location: &ProjectLocation,
) -> Result<(), CliError> {
    check_api_running(sdk).await?;

    let project_name = project_location.get_project_name();

    if sdk.is_authenticated() {
        if sdk.refresh_auth().await.is_ok() {
            if let Some(token_data) = load_token_from_cache(&project_name, account)? {
                save_token_to_cache(sdk, &project_name, account, token_data.address)?;
            }
            return Ok(());
        }
    }

    if let Some(token_data) = load_token_from_cache(&project_name, account)? {
        sdk.update_auth_token(token_data.token_pair.clone());

        if sdk.refresh_auth().await.is_ok() {
            save_token_to_cache(sdk, &project_name, account, token_data.address)?;
            return Ok(());
        }

        sdk.context.token_pair = None;
    }

    let password_manager = KeyStorePasswordManager::new(&project_name).map_err(|e| {
        CliError::Authentication(format!("Failed to create password manager: {}", e))
    })?;
    let keystore_path = project_location.get_account_keystore(account);

    let password = match password_manager.load(account) {
        Ok(pwd) => pwd,
        Err(PasswordError::NotFound) => {
            let pwd = Password::new()
                .with_prompt(format!("Enter password for account '{}'", account))
                .interact()?;

            match decrypt_keystore(&keystore_path, &pwd) {
                Ok(_) => {
                    password_manager.save(account, &pwd).map_err(|e| {
                        CliError::Authentication(format!("Failed to save password: {}", e))
                    })?;
                    pwd
                }
                Err(_) => {
                    return Err(CliError::Authentication(
                        "Invalid password or keystore not found".to_string(),
                    ));
                }
            }
        }
        Err(e) => return Err(CliError::Authentication(format!("Error loading password: {}", e))),
    };

    let keystore_result = decrypt_keystore(&keystore_path, &password)?;

    match keystore_result {
        KeystoreDecryptResult::PrivateKey { hex_key, address, .. } => {
            let private_key = B256::from_str(&hex_key)
                .map_err(|e| CliError::Internal(format!("Invalid hex key: {}", e)))?;
            let signer = PrivateKeySigner::from_bytes(&private_key)
                .map_err(|e| CliError::Internal(format!("Failed to create signer: {}", e)))?;
            let challenge_result = sdk
                .get_auth_challenge(
                    &Address::parse_checksummed(&address, None)
                        .map_err(|e| CliError::AddressParse(format!("Invalid address: {}", e)))?,
                )
                .await
                .map_err(|e| CliError::Api(format!("Failed to get auth challenge: {}", e)))?;

            let signature = signer
                .sign_message(challenge_result.challenge.as_bytes())
                .await
                .map_err(|e| CliError::Internal(format!("Failed to sign message: {}", e)))?;

            sdk.login(&challenge_result, signature)
                .await
                .map_err(|e| CliError::Api(format!("Login failed: {}", e)))?;

            save_token_to_cache(sdk, &project_name, account, address)?;

            Ok(())
        }
        KeystoreDecryptResult::Mnemonic { .. } => Err(CliError::Authentication(
            "Mnemonic-based accounts are not supported for authentication".to_string(),
        )),
    }
}
