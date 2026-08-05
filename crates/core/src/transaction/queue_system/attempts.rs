use async_trait::async_trait;

use alloy::transports::{RpcError, TransportErrorKind};

use crate::{
    provider::EvmProvider,
    transaction::{db::RecordedTransactionAttempt, types::TransactionHash},
};

use super::transactions_queue::TransactionsQueue;

#[async_trait]
pub(super) trait TransactionExistenceChecker {
    async fn transaction_exists(
        &self,
        hash: &TransactionHash,
    ) -> Result<bool, RpcError<TransportErrorKind>>;
}

#[async_trait]
impl TransactionExistenceChecker for EvmProvider {
    async fn transaction_exists(
        &self,
        hash: &TransactionHash,
    ) -> Result<bool, RpcError<TransportErrorKind>> {
        EvmProvider::transaction_exists(self, hash).await
    }
}

#[async_trait]
impl TransactionExistenceChecker for TransactionsQueue {
    async fn transaction_exists(
        &self,
        hash: &TransactionHash,
    ) -> Result<bool, RpcError<TransportErrorKind>> {
        TransactionsQueue::transaction_exists(self, hash).await
    }
}

pub(super) async fn find_landed_attempt<'a, C: TransactionExistenceChecker>(
    checker: &C,
    attempts: &'a [RecordedTransactionAttempt],
) -> Result<Option<&'a RecordedTransactionAttempt>, RpcError<TransportErrorKind>> {
    let mut landed = None;

    for attempt in attempts {
        if checker.transaction_exists(&attempt.hash).await? && landed.is_none() {
            landed = Some(attempt);
        }
    }

    Ok(landed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::TxHash;
    use std::collections::HashSet;

    struct TestChecker {
        landed: HashSet<TransactionHash>,
    }

    #[async_trait]
    impl TransactionExistenceChecker for TestChecker {
        async fn transaction_exists(
            &self,
            hash: &TransactionHash,
        ) -> Result<bool, RpcError<TransportErrorKind>> {
            Ok(self.landed.contains(hash))
        }
    }

    fn attempt(byte: u8) -> RecordedTransactionAttempt {
        RecordedTransactionAttempt {
            hash: TransactionHash::new(TxHash::repeat_byte(byte)),
            sent_with_gas: None,
            sent_with_blob_gas: None,
        }
    }

    #[tokio::test]
    async fn recovery_returns_first_landed_attempt_in_recorded_order() {
        let attempts = vec![attempt(1), attempt(2), attempt(3)];
        let checker = TestChecker { landed: HashSet::from([attempts[1].hash, attempts[2].hash]) };

        let landed = find_landed_attempt(&checker, &attempts).await.unwrap().unwrap();

        assert_eq!(landed.hash, attempts[1].hash);
    }

    #[tokio::test]
    async fn recovery_returns_none_only_when_every_recorded_attempt_is_absent() {
        let attempts = vec![attempt(1), attempt(2)];
        let checker = TestChecker { landed: HashSet::new() };

        assert!(find_landed_attempt(&checker, &attempts).await.unwrap().is_none());
    }
}
