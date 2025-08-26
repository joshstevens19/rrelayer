use tokio::sync::Mutex;

use crate::transaction::types::TransactionNonce;

/// Manages transaction nonces with thread-safe increment operations.
///
/// The NonceManager ensures that nonce values are incremented atomically
/// to prevent race conditions when multiple threads are creating transactions.
pub struct NonceManager {
    nonce: TransactionNonce,
    lock: Mutex<()>,
}

impl NonceManager {
    /// Creates a new NonceManager with the given starting nonce.
    ///
    /// # Arguments
    /// * `current_nonce` - The initial nonce value to start from
    ///
    /// # Returns
    /// * `NonceManager` - A new instance with the specified starting nonce
    pub fn new(current_nonce: TransactionNonce) -> Self {
        NonceManager { nonce: current_nonce, lock: Mutex::new(()) }
    }

    /// Increments the nonce value by 1 in a thread-safe manner.
    ///
    /// This method acquires a lock to ensure that nonce increments are atomic,
    /// preventing race conditions when multiple threads are incrementing the nonce.
    pub async fn increase(&mut self) {
        let _lock = self.lock.lock().await;
        self.nonce = self.nonce + 1;
    }

    /// Returns the current nonce value.
    ///
    /// # Returns
    /// * `TransactionNonce` - The current nonce value
    pub fn current(&self) -> TransactionNonce {
        self.nonce
    }
}
