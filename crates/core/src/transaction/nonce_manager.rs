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

    /// Undoes a [`Self::get_and_increment`] whose reserved nonce was never persisted.
    ///
    /// Only moves backward when the manager's next nonce is still `assigned + 1`.
    /// A later reservation (or an on-chain sync that moved past it) is left alone,
    /// so a failed save cannot rewind a nonce something else has already taken.
    pub async fn rollback_increment(&self, assigned: TransactionNonce) {
        let mut nonce_guard = self.nonce.lock().await;
        if nonce_guard.into_inner() == assigned.into_inner().saturating_add(1) {
            *nonce_guard = assigned;
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rollback_increment_restores_only_an_uncommitted_reservation() {
        let manager = NonceManager::new(TransactionNonce::new(5));

        let assigned = manager.get_and_increment().await;
        assert_eq!(assigned.into_inner(), 5);
        manager.rollback_increment(assigned).await;
        assert_eq!(manager.get_current_nonce().await.into_inner(), 5);

        let first = manager.get_and_increment().await;
        let _second = manager.get_and_increment().await;
        // A later reservation has moved past `first`; rolling it back must not
        // rewind the nonce the second reservation already took.
        manager.rollback_increment(first).await;
        assert_eq!(manager.get_current_nonce().await.into_inner(), 7);
    }
}
