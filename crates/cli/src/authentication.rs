use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
    time::{Duration, SystemTime},
};

use alloy::{
    primitives::{Address, B256},
    signers::{local::PrivateKeySigner, Signer},
};
use dialoguer::Password;
use rrelayerr_core::{
    authentication::types::TokenPair,
    keystore::{decrypt_keystore, KeyStorePasswordManager, KeystoreDecryptResult, PasswordError},
};
use rrelayerr_sdk::SDK;
use serde::{Deserialize, Serialize};

use crate::commands::keystore::ProjectLocation;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredTokenData {
    token_pair: TokenPair,
    address: String,
    expires_at: SystemTime,
}

fn get_secure_token_path(project_name: &str, account: &str) -> PathBuf {
    let base_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rrelayerr")
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

fn save_token_to_cache(
    sdk: &SDK,
    project_name: &str,
    account: &str,
    address: String,
) -> Result<(), Box<dyn std::error::Error>> {
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
        Err("No token available to save".into())
    }
}

fn load_token_from_cache(
    project_name: &str,
    account: &str,
) -> Result<Option<StoredTokenData>, Box<dyn std::error::Error>> {
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

pub async fn check_api_running(sdk: &SDK) -> Result<(), Box<dyn std::error::Error>> {
    match sdk.health.check().await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error: API server is not running or is unreachable.");
            eprintln!("Please start the API server before continuing.");
            eprintln!("Details: {}", e);

            Err("The API server is not running. Please start it before continuing.".into())
        }
    }
}

pub async fn handle_authenticate(
    sdk: &mut SDK,
    account: &str,
    project_location: &ProjectLocation,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let password_manager = KeyStorePasswordManager::new(&project_name);
    let keystore_path = project_location.get_account_keystore(account);

    let password = match password_manager.load(account) {
        Ok(pwd) => pwd,
        Err(PasswordError::NotFound) => {
            let pwd = Password::new()
                .with_prompt(format!("Enter password for account '{}'", account))
                .interact()?;

            match decrypt_keystore(&keystore_path, &pwd) {
                Ok(_) => {
                    password_manager.save(account, &pwd)?;
                    pwd
                }
                Err(_) => return Err("Invalid password or keystore not found".into()),
            }
        }
        Err(e) => return Err(format!("Error loading password: {}", e).into()),
    };

    let keystore_result = decrypt_keystore(&keystore_path, &password)?;

    match keystore_result {
        KeystoreDecryptResult::PrivateKey { hex_key, address, .. } => {
            let private_key = B256::from_str(&hex_key)?;
            let signer = PrivateKeySigner::from_bytes(&private_key)?;
            let challenge_result =
                sdk.get_auth_challenge(&Address::parse_checksummed(&address, None)?).await?;

            let signature = signer.sign_message(challenge_result.challenge.as_bytes()).await?;

            sdk.login(&challenge_result, signature).await?;

            save_token_to_cache(sdk, &project_name, account, address)?;
            
            Ok(())
        }
        KeystoreDecryptResult::Mnemonic { .. } => {
            Err("Mnemonic-based accounts are not supported for authentication".into())
        }
    }
}
