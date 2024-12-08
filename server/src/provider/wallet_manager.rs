use std::collections::HashMap;

use alloy::signers::{
    local::{coins_bip39::English, LocalSignerError, MnemonicBuilder, PrivateKeySigner},
    Signer,
};
use tokio::sync::Mutex;

use crate::network::types::ChainId;

pub struct WalletManager {
    wallets: Mutex<HashMap<u32, PrivateKeySigner>>,
    mnemonic: String,
}

impl WalletManager {
    pub fn new(mnemonic: &str) -> Self {
        WalletManager { wallets: Mutex::new(HashMap::new()), mnemonic: mnemonic.to_string() }
    }

    pub async fn get_wallet(
        &self,
        index: u32,
        chain_id: &ChainId,
    ) -> Result<PrivateKeySigner, LocalSignerError> {
        let mut wallets = self.wallets.lock().await;

        if let Some(wallet) = wallets.get(&index) {
            Ok(wallet.clone())
        } else {
            let wallet = MnemonicBuilder::<English>::default()
                .phrase::<&str>(&self.mnemonic)
                .index(index)?
                .build()?
                .with_chain_id(Some(chain_id.0));

            wallets.insert(index, wallet.clone());

            Ok(wallet)
        }
    }
}
