use std::{fs, io::Write, path::PathBuf};

use alloy::signers::{
    k256::ecdsa::SigningKey,
    local::{coins_bip39::English, LocalSigner, MnemonicBuilder, PrivateKeySigner},
};
use base64::{engine::general_purpose, Engine as _};
use eth_keystore::{decrypt_key, encrypt_key};
use rand::thread_rng;
use thiserror::Error;

use crate::wallet::{generate_seed_phrase, WalletError};

/// Creates a new mnemonic phrase and stores it in an encrypted keystore file.
///
/// This function generates a new 24-word BIP39 mnemonic phrase and encrypts it
/// using the provided password, then saves it to a keystore file.
///
/// # Arguments
/// * `password` - Password to use for keystore encryption
/// * `output_path` - Directory path where the keystore file will be created
/// * `filename` - Name for the keystore file
///
/// # Returns
/// * `Ok(())` - If the mnemonic was successfully generated and stored
/// * `Err(WalletError)` - If mnemonic generation or keystore creation fails
pub fn create_new_mnemonic_in_keystore(
    password: &str,
    output_path: &PathBuf,
    filename: &str,
) -> Result<(), WalletError> {
    let phrase = generate_seed_phrase()?;

    store_mnemonic_in_keystore(&phrase, password, output_path, filename)
}

/// Stores an existing mnemonic phrase in an encrypted keystore file.
///
/// This function takes an existing mnemonic phrase and encrypts it using the
/// provided password, then saves it to a keystore file.
///
/// # Arguments
/// * `phrase` - The mnemonic phrase to store
/// * `password` - Password to use for keystore encryption
/// * `output_path` - Directory path where the keystore file will be created
/// * `filename` - Name for the keystore file
///
/// # Returns
/// * `Ok(())` - If the mnemonic was successfully stored
/// * `Err(WalletError)` - If keystore creation fails
pub fn store_mnemonic_in_keystore(
    phrase: &str,
    password: &str,
    output_path: &PathBuf,
    filename: &str,
) -> Result<(), WalletError> {
    let mut rng = thread_rng();
    encrypt_key(output_path, &mut rng, phrase.as_bytes(), password, Some(filename))?;

    Ok(())
}

/// Recovers a mnemonic phrase from an encrypted keystore file.
///
/// This function decrypts a keystore file using the provided password
/// and returns the stored mnemonic phrase.
///
/// # Arguments
/// * `keystore_path` - Path to the keystore file
/// * `password` - Password used to encrypt the keystore
///
/// # Returns
/// * `Ok(String)` - The recovered mnemonic phrase
/// * `Err(WalletError)` - If decryption fails or the file cannot be read
pub fn recover_mnemonic_from_keystore(
    keystore_path: &PathBuf,
    password: &str,
) -> Result<String, WalletError> {
    let mnemonic_bytes = decrypt_key(keystore_path, password)?;

    Ok(String::from_utf8(mnemonic_bytes)?)
}

/// Creates a new random private key and stores it in an encrypted keystore file.
///
/// This function generates a new random private key and encrypts it using
/// the provided password, then saves it to a keystore file.
///
/// # Arguments
/// * `password` - Password to use for keystore encryption
/// * `output_path` - Directory path where the keystore file will be created
/// * `filename` - Name for the keystore file
///
/// # Returns
/// * `Ok(())` - If the private key was successfully generated and stored
/// * `Err(WalletError)` - If key generation or keystore creation fails
pub fn create_new_private_key_in_keystore(
    password: &str,
    output_path: &PathBuf,
    filename: &str,
) -> Result<(), WalletError> {
    let private_key = PrivateKeySigner::random();

    store_private_key_in_keystore(private_key, password, output_path, Some(filename))
}

/// Stores an existing private key in an encrypted keystore file.
///
/// This function takes an existing private key and encrypts it using the
/// provided password, then saves it to a keystore file.
///
/// # Arguments
/// * `private_key` - The private key signer to store
/// * `password` - Password to use for keystore encryption
/// * `output_path` - Directory path where the keystore file will be created
/// * `filename` - Optional name for the keystore file (auto-generated if None)
///
/// # Returns
/// * `Ok(())` - If the private key was successfully stored
/// * `Err(WalletError)` - If keystore creation fails
pub fn store_private_key_in_keystore(
    private_key: PrivateKeySigner,
    password: &str,
    output_path: &PathBuf,
    filename: Option<&str>,
) -> Result<(), WalletError> {
    let mut rng = thread_rng();
    encrypt_key(output_path, &mut rng, private_key.to_bytes(), password, filename)?;

    Ok(())
}

/// Recovers a wallet signer from an encrypted keystore file containing a private key.
///
/// This function decrypts a keystore file containing a private key and creates
/// a LocalSigner that can be used for signing transactions.
///
/// # Arguments
/// * `keystore_path` - Path to the keystore file
/// * `password` - Password used to encrypt the keystore
///
/// # Returns
/// * `Ok(LocalSigner<SigningKey>)` - A wallet signer created from the private key
/// * `Err(WalletError)` - If decryption fails or wallet creation fails
pub fn recover_wallet_from_keystore(
    keystore_path: &PathBuf,
    password: &str,
) -> Result<LocalSigner<SigningKey>, WalletError> {
    let private_key = decrypt_key(keystore_path, password)?;

    let wallet = LocalSigner::from_slice(&private_key).map_err(|e| {
        WalletError::KeyDerivationError(format!("Failed to create wallet from private key: {}", e))
    })?;

    Ok(wallet)
}

pub enum KeystoreDecryptResult {
    Mnemonic { phrase: String, address: String },
    PrivateKey { key: Vec<u8>, hex_key: String, address: String },
}

/// Decrypts a keystore file and determines whether it contains a mnemonic or private key.
///
/// This function attempts to decrypt a keystore file and automatically determines
/// whether the stored data is a mnemonic phrase or a private key. It returns the
/// appropriate result type with the decrypted data and associated wallet address.
///
/// # Arguments
/// * `keystore_path` - Path to the keystore file
/// * `password` - Password used to encrypt the keystore
///
/// # Returns
/// * `Ok(KeystoreDecryptResult::Mnemonic)` - If the keystore contains a valid mnemonic
/// * `Ok(KeystoreDecryptResult::PrivateKey)` - If the keystore contains a valid private key
/// * `Err(WalletError)` - If decryption fails or the data is neither a valid mnemonic nor private key
pub fn decrypt_keystore(
    keystore_path: &PathBuf,
    password: &str,
) -> Result<KeystoreDecryptResult, WalletError> {
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
        Err(_) => Err(WalletError::ConfigurationError {
            message: "Could not determine keystore type - not a valid mnemonic or private key"
                .to_string(),
        }),
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
    /// Creates a new KeyStorePasswordManager instance.
    ///
    /// This creates the password storage directory (.rrelayer/accounts) in the user's
    /// home directory if it doesn't already exist.
    ///
    /// # Arguments
    /// * `app_name` - Name of the application (used as prefix for password files)
    ///
    /// # Returns
    /// * `Ok(Self)` - A new KeyStorePasswordManager instance
    /// * `Err(std::io::Error)` - If the home directory cannot be found or storage directory cannot be created
    pub fn new(app_name: &str) -> Result<Self, std::io::Error> {
        // Create .rrelayer/accounts directory in user's home directory
        let home_dir = dirs::home_dir().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Could not find home directory")
        })?;
        let storage_dir = home_dir.join(".rrelayer").join("accounts");

        fs::create_dir_all(&storage_dir)?;

        Ok(Self { app_name: app_name.to_string(), storage_dir })
    }

    /// Constructs the file path for storing a password with the given key.
    ///
    /// # Arguments
    /// * `key` - The key identifier for the password
    ///
    /// # Returns
    /// * `PathBuf` - The full path where the password file should be stored
    fn get_password_path(&self, key: &str) -> PathBuf {
        self.storage_dir.join(format!("{}-{}.pwd", self.app_name, key))
    }

    /// Saves a password to an encrypted file.
    ///
    /// The password is base64 encoded before being written to disk for basic obfuscation.
    ///
    /// # Arguments
    /// * `key` - The key identifier for the password
    /// * `password` - The password to save
    ///
    /// # Returns
    /// * `Ok(())` - If the password was successfully saved
    /// * `Err(PasswordError)` - If file writing fails
    pub fn save(&self, key: &str, password: &str) -> Result<(), PasswordError> {
        let password_path = self.get_password_path(key);

        let encoded = general_purpose::STANDARD.encode(password.as_bytes());

        fs::write(password_path, encoded).map_err(PasswordError::IoError)?;

        Ok(())
    }

    /// Loads a password from an encrypted file.
    ///
    /// The password is base64 decoded after reading from disk.
    ///
    /// # Arguments
    /// * `key` - The key identifier for the password
    ///
    /// # Returns
    /// * `Ok(String)` - The decrypted password
    /// * `Err(PasswordError)` - If the file doesn't exist, cannot be read, or decoding fails
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

    /// Deletes a stored password file.
    ///
    /// # Arguments
    /// * `key` - The key identifier for the password to delete
    ///
    /// # Returns
    /// * `Ok(())` - If the password file was successfully deleted
    /// * `Err(PasswordError)` - If the file doesn't exist or deletion fails
    pub fn delete(&self, key: &str) -> Result<(), PasswordError> {
        let password_path = self.get_password_path(key);

        if !password_path.exists() {
            return Err(PasswordError::NotFound);
        }

        fs::remove_file(password_path).map_err(PasswordError::IoError)?;
        Ok(())
    }

    /// Lists all account keys that have stored passwords.
    ///
    /// This scans the password storage directory and returns all account keys
    /// that have associated password files.
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - A vector of account key names
    /// * `Err(PasswordError)` - If the storage directory cannot be read
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
                    if let (Some(without_prefix), Some(without_suffix)) =
                        (filename.strip_prefix(&prefix), filename.strip_suffix(suffix))
                    {
                        let account_name = without_prefix.to_string();
                        accounts.push(account_name);
                    }
                }
            }
        }

        Ok(accounts)
    }
}
