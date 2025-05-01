use std::{fs, io::Write, path::PathBuf};

use alloy::signers::{
    k256::ecdsa::SigningKey,
    local::{coins_bip39::English, LocalSigner, MnemonicBuilder, PrivateKeySigner},
};
use base64::{engine::general_purpose, Engine as _};
use eth_keystore::{decrypt_key, encrypt_key};
use rand::thread_rng;
use thiserror::Error;

use crate::generate_seed_phrase;

pub fn create_new_mnemonic_in_keystore(
    password: &str,
    output_path: &PathBuf,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let phrase = generate_seed_phrase()?;

    store_mnemonic_in_keystore(&phrase, password, output_path, filename)
}

pub fn store_mnemonic_in_keystore(
    phrase: &str,
    password: &str,
    output_path: &PathBuf,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = thread_rng();
    encrypt_key(output_path, &mut rng, phrase.as_bytes(), password, Some(filename))?;

    Ok(())
}

pub fn recover_mnemonic_from_keystore(
    keystore_path: &PathBuf,
    password: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mnemonic_bytes = decrypt_key(keystore_path, password)?;

    Ok(String::from_utf8(mnemonic_bytes)?)
}

pub fn create_new_private_key_in_keystore(
    password: &str,
    output_path: &PathBuf,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let private_key = PrivateKeySigner::random();

    store_private_key_in_keystore(private_key, password, output_path, Some(filename))
}

pub fn store_private_key_in_keystore(
    private_key: PrivateKeySigner,
    password: &str,
    output_path: &PathBuf,
    filename: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = thread_rng();
    encrypt_key(output_path, &mut rng, private_key.to_bytes(), password, filename)?;

    Ok(())
}

pub fn recover_wallet_from_keystore(
    keystore_path: &PathBuf,
    password: &str,
) -> Result<LocalSigner<SigningKey>, Box<dyn std::error::Error + Send + Sync>> {
    let private_key = decrypt_key(keystore_path, password)?;

    let wallet = LocalSigner::from_slice(&private_key)?;

    Ok(wallet)
}

pub enum KeystoreDecryptResult {
    Mnemonic { phrase: String, address: String },
    PrivateKey { key: Vec<u8>, hex_key: String, address: String },
}

pub fn decrypt_keystore(
    keystore_path: &PathBuf,
    password: &str,
) -> Result<KeystoreDecryptResult, Box<dyn std::error::Error>> {
    let bytes = decrypt_key(keystore_path, password)?;

    if let Ok(phrase) = String::from_utf8(bytes.clone()) {
        if let Ok(wallet) = MnemonicBuilder::<English>::default().phrase(&phrase).build() {
            return Ok(KeystoreDecryptResult::Mnemonic {
                phrase,
                address: wallet.address().to_string(),
            });
        }
    }

    match LocalSigner::from_slice(&bytes) {
        Ok(wallet) => {
            let hex_key = format!("0x{}", hex::encode(&bytes));
            Ok(KeystoreDecryptResult::PrivateKey {
                key: bytes,
                hex_key,
                address: wallet.address().to_string(),
            })
        }
        Err(_) => {
            Err("Could not determine keystore type - not a valid mnemonic or private key".into())
        }
    }
}

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Password not found")]
    NotFound,

    #[error("Encoding error")]
    EncodingError,
}

pub struct KeyStorePasswordManager {
    app_name: String,
    storage_dir: PathBuf,
}

impl KeyStorePasswordManager {
    pub fn new(app_name: &str) -> Self {
        // Create .rrelayerr/accounts directory in user's home directory
        let home_dir = dirs::home_dir().expect("Could not find home directory");
        let storage_dir = home_dir.join(".rrelayerr").join("accounts");

        fs::create_dir_all(&storage_dir).expect("Failed to create account storage directory");

        Self { app_name: app_name.to_string(), storage_dir }
    }

    fn get_password_path(&self, key: &str) -> PathBuf {
        self.storage_dir.join(format!("{}-{}.pwd", self.app_name, key))
    }

    pub fn save(&self, key: &str, password: &str) -> Result<(), PasswordError> {
        let password_path = self.get_password_path(key);

        let encoded = general_purpose::STANDARD.encode(password.as_bytes());

        fs::write(password_path, encoded).map_err(PasswordError::IoError)?;

        Ok(())
    }

    pub fn load(&self, key: &str) -> Result<String, PasswordError> {
        let password_path = self.get_password_path(key);

        if !password_path.exists() {
            println!("DEBUG: Password file not found for key '{}'", key);
            return Err(PasswordError::NotFound);
        }

        let encoded = fs::read_to_string(&password_path).map_err(PasswordError::IoError)?;

        let decoded =
            general_purpose::STANDARD.decode(encoded).map_err(|_| PasswordError::EncodingError)?;

        let password = String::from_utf8(decoded).map_err(|_| PasswordError::EncodingError)?;

        Ok(password)
    }

    pub fn delete(&self, key: &str) -> Result<(), PasswordError> {
        let password_path = self.get_password_path(key);

        if !password_path.exists() {
            return Err(PasswordError::NotFound);
        }

        fs::remove_file(password_path).map_err(PasswordError::IoError)?;
        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<String>, PasswordError> {
        let entries = match fs::read_dir(&self.storage_dir) {
            Ok(entries) => entries,
            Err(e) => return Err(PasswordError::IoError(e)),
        };

        let mut accounts = Vec::new();
        let prefix = format!("{}-", self.app_name);
        let suffix = ".pwd";

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            if let Some(filename) = entry.file_name().to_str() {
                if filename.starts_with(&prefix) && filename.ends_with(suffix) {
                    let account_name = filename
                        .strip_prefix(&prefix)
                        .unwrap()
                        .strip_suffix(suffix)
                        .unwrap()
                        .to_string();

                    accounts.push(account_name);
                }
            }
        }

        Ok(accounts)
    }
}
