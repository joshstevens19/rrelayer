use tokio::sync::Mutex;

use crate::transaction::types::TransactionNonce;

pub struct NonceManager {
    nonce: Mutex<TransactionNonce>,
}

impl NonceManager {
    pub fn new(current_nonce: TransactionNonce) -> Self {
        NonceManager { nonce: Mutex::new(current_nonce) }
    }

    pub async fn get_and_increment(&self) -> TransactionNonce {
        let mut nonce_guard = self.nonce.lock().await;
        let current_nonce = *nonce_guard;
        *nonce_guard = current_nonce + 1;
        current_nonce
    }
}
