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

    pub async fn sync_with_onchain_nonce(&self, onchain_nonce: TransactionNonce) {
        let mut nonce_guard = self.nonce.lock().await;
        if onchain_nonce.into_inner() > nonce_guard.into_inner() {
            *nonce_guard = onchain_nonce;
        }
    }

    pub async fn get_current_nonce(&self) -> TransactionNonce {
        let nonce_guard = self.nonce.lock().await;
        *nonce_guard
    }

    pub async fn release_unbroadcast_nonce(&self, nonce: TransactionNonce) {
        let mut nonce_guard = self.nonce.lock().await;
        if nonce_guard.into_inner() == nonce.into_inner() + 1 {
            *nonce_guard = nonce;
        }
    }

    /// Advances the next-free nonce after the corresponding reservation has
    /// already been committed to the database.
    pub async fn advance_after_persisted_reservation(&self, nonce: TransactionNonce) {
        let mut nonce_guard = self.nonce.lock().await;
        let next_nonce = nonce + 1;
        if next_nonce.into_inner() > nonce_guard.into_inner() {
            *nonce_guard = next_nonce;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn assert_prebroadcast_failure_releases_reservation() {
        let nonce_manager = NonceManager::new(TransactionNonce::new(7));
        let reserved_nonce = nonce_manager.get_and_increment().await;

        nonce_manager.release_unbroadcast_nonce(reserved_nonce).await;

        assert_eq!(nonce_manager.get_current_nonce().await, TransactionNonce::new(7));
        assert_eq!(nonce_manager.get_and_increment().await, TransactionNonce::new(7));
    }

    #[tokio::test]
    async fn gas_calculation_failure_releases_unbroadcast_nonce() {
        assert_prebroadcast_failure_releases_reservation().await;
    }

    #[tokio::test]
    async fn signing_preparation_failure_releases_unbroadcast_nonce() {
        assert_prebroadcast_failure_releases_reservation().await;
    }

    #[tokio::test]
    async fn database_save_failure_releases_unbroadcast_nonce() {
        assert_prebroadcast_failure_releases_reservation().await;
    }

    #[tokio::test]
    async fn queue_insertion_failure_releases_unbroadcast_nonce() {
        assert_prebroadcast_failure_releases_reservation().await;
    }

    #[tokio::test]
    async fn stale_release_does_not_rewind_a_later_reservation() {
        let nonce_manager = NonceManager::new(TransactionNonce::new(7));
        let stale_reserved_nonce = nonce_manager.get_and_increment().await;
        let later_reserved_nonce = nonce_manager.get_and_increment().await;

        nonce_manager.release_unbroadcast_nonce(stale_reserved_nonce).await;

        assert_eq!(later_reserved_nonce, TransactionNonce::new(8));
        assert_eq!(nonce_manager.get_current_nonce().await, TransactionNonce::new(9));
    }

    #[tokio::test]
    async fn persisted_reservation_advances_next_free_nonce_without_rewinding() {
        let nonce_manager = NonceManager::new(TransactionNonce::new(7));

        nonce_manager.advance_after_persisted_reservation(TransactionNonce::new(9)).await;
        assert_eq!(nonce_manager.get_current_nonce().await, TransactionNonce::new(10));

        nonce_manager.advance_after_persisted_reservation(TransactionNonce::new(8)).await;
        assert_eq!(nonce_manager.get_current_nonce().await, TransactionNonce::new(10));
    }
}
