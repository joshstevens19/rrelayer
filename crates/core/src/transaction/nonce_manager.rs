use tokio::sync::Mutex;

use crate::transaction::types::TransactionNonce;

pub struct NonceManager {
    nonce: TransactionNonce,
    lock: Mutex<()>,
}

impl NonceManager {
    pub fn new(current_nonce: TransactionNonce) -> Self {
        NonceManager { nonce: current_nonce, lock: Mutex::new(()) }
    }

    pub async fn increase(&mut self) {
        let _lock = self.lock.lock().await;
        self.nonce = self.nonce + 1;
    }

    pub fn current(&self) -> TransactionNonce {
        self.nonce
    }
}
