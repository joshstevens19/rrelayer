use std::path::PathBuf;

use alloy::signers::{
    k256::ecdsa::SigningKey,
    local::{coins_bip39::English, LocalSigner, MnemonicBuilder, PrivateKeySigner},
};
use eth_keystore::{decrypt_key, encrypt_key};
use rand::thread_rng;

pub fn create_new_mnemonic_in_keystore(
    password: &str,
    output_path: &PathBuf,
    filename: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wallet =
        MnemonicBuilder::<English>::default().word_count(24).derivation_path("m/44'/60'/0'/2/1")?;

    store_mnemonic_in_keystore(&wallet, password, output_path, filename)
}

pub fn store_mnemonic_in_keystore(
    mnemonic: &MnemonicBuilder,
    password: &str,
    output_path: &PathBuf,
    filename: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wallet = mnemonic.build()?;

    let mut rng = thread_rng();
    encrypt_key(output_path, &mut rng, &wallet.to_bytes(), password, filename)?;

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
    filename: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let private_key = PrivateKeySigner::random();

    store_private_key_in_keystore(private_key, password, output_path, filename)
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
