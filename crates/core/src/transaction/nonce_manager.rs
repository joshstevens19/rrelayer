use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::transaction::types::TransactionNonce;

pub struct NonceManager {
    nonce: Arc<Mutex<TransactionNonce>>,
}

pub struct NonceReservation {
    nonce: TransactionNonce,
    committed: bool,
    guard: Option<OwnedMutexGuard<TransactionNonce>>,
}

impl NonceManager {
    pub fn new(current_nonce: TransactionNonce) -> Self {
        NonceManager { nonce: Arc::new(Mutex::new(current_nonce)) }
    }

    /// Reserves and increments the next nonce while holding the internal mutex until commit or drop.
    ///
    /// Callers should keep the work between reservation and `NonceReservation::commit` short; if
    /// the reservation is dropped before commit, the nonce cursor is rolled back automatically.
    pub async fn reserve_next(&self) -> NonceReservation {
        let mut nonce_guard = self.nonce.clone().lock_owned().await;
        let current_nonce = *nonce_guard;
        *nonce_guard = current_nonce + 1;
        NonceReservation { nonce: current_nonce, committed: false, guard: Some(nonce_guard) }
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

impl NonceReservation {
    pub fn nonce(&self) -> TransactionNonce {
        self.nonce
    }

    pub fn commit(mut self) {
        self.committed = true;
        self.guard.take();
    }
}

impl Drop for NonceReservation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        if let Some(mut nonce_guard) = self.guard.take() {
            *nonce_guard = self.nonce;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NonceManager;
    use crate::transaction::types::TransactionNonce;

    #[tokio::test]
    async fn reservation_rolls_back_when_dropped() {
        let manager = NonceManager::new(TransactionNonce::new(7));

        {
            let reservation = manager.reserve_next().await;
            assert_eq!(reservation.nonce(), TransactionNonce::new(7));
        }

        assert_eq!(manager.get_current_nonce().await, TransactionNonce::new(7));
    }

    #[tokio::test]
    async fn reservation_commit_keeps_increment() {
        let manager = NonceManager::new(TransactionNonce::new(7));

        let reservation = manager.reserve_next().await;
        assert_eq!(reservation.nonce(), TransactionNonce::new(7));
        reservation.commit();

        assert_eq!(manager.get_current_nonce().await, TransactionNonce::new(8));
    }
}
