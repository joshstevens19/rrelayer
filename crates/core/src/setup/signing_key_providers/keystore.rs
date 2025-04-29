use std::path::PathBuf;

use alloy::signers::{
    k256::ecdsa::SigningKey,
    local::{
        coins_bip39::{English, Mnemonic},
        LocalSigner, MnemonicBuilder, PrivateKeySigner,
    },
};
use eth_keystore::{decrypt_key, encrypt_key};
use rand::thread_rng;

pub fn create_new_mnemonic_in_keystore(
    password: &str,
    output_path: &PathBuf,
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Generate a new random 24-word mnemonic
    let mnemonic = Mnemonic::<English>::new_with_count(&mut thread_rng(), 24)?;
    let phrase = mnemonic.to_phrase();

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
) -> Result<MnemonicBuilder, Box<dyn std::error::Error>> {
    let mnemonic_bytes = decrypt_key(keystore_path, password)?;

    let mnemonic = String::from_utf8(mnemonic_bytes)?;

    Ok(MnemonicBuilder::<English>::default().phrase(&mnemonic))
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
) -> Result<LocalSigner<SigningKey>, Box<dyn std::error::Error>> {
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
        // Validate the phrase is actually a mnemonic
        if let Ok(wallet) = MnemonicBuilder::<English>::default().phrase(&phrase).build() {
            // It's a valid mnemonic
            return Ok(KeystoreDecryptResult::Mnemonic {
                phrase,
                address: wallet.address().to_string(),
            });
        }
    }

    // Second approach: try to treat it as a private key
    match LocalSigner::from_slice(&bytes) {
        Ok(wallet) => {
            // It's a valid private key
            let hex_key = format!("0x{}", hex::encode(&bytes));
            Ok(KeystoreDecryptResult::PrivateKey {
                key: bytes,
                hex_key,
                address: wallet.address().to_string(),
            })
        }
        Err(_) => {
            // Could not determine the type - return a meaningful error
            Err("Could not determine keystore type - not a valid mnemonic or private key".into())
        }
    }
}
