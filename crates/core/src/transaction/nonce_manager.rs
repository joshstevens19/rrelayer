use tokio::sync::Mutex;

use crate::transaction::types::TransactionNonce;

/// Manages transaction nonces with thread-safe increment operations.
///
/// The NonceManager ensures that nonce values are incremented atomically
/// to prevent race conditions when multiple threads are creating transactions.
pub struct NonceManager {
    nonce: Mutex<TransactionNonce>,
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
        NonceManager { nonce: Mutex::new(current_nonce) }
    }

    /// Gets the current nonce value and increments it atomically.
    ///
    /// This method ensures that nonce allocation is atomic - the returned nonce
    /// value is guaranteed to be unique and the internal counter is incremented
    /// in a single locked operation, preventing race conditions.
    ///
    /// # Returns
    /// * `TransactionNonce` - The nonce value to use for the transaction
    pub async fn get_and_increment(&self) -> TransactionNonce {
        let mut nonce_guard = self.nonce.lock().await;
        let current_nonce = *nonce_guard;
        *nonce_guard = current_nonce + 1;
        current_nonce
    }
}
