use crate::{
    gas::{fee_estimator::base::GasPriceResult, types::GasLimit},
    postgres::{PostgresClient, PostgresError},
    relayer::types::RelayerId,
    shared::{
        common_types::{BlockHash, BlockNumber},
        utils::option_if,
    },
    transaction::types::{Transaction, TransactionHash, TransactionId, TransactionStatus},
};
use alloy::network::AnyTransactionReceipt;
use chrono::Utc;

const TRANSACTION_TABLES: [&str; 2] = ["relayer.transaction", "relayer.transaction_audit_log"];

impl PostgresClient {
    /// Saves a new transaction to the database.
    ///
    /// Inserts the transaction into both the main transaction table and the audit log
    /// within a database transaction to ensure consistency.
    ///
    /// # Arguments
    /// * `relayer_id` - The ID of the relayer handling this transaction
    /// * `transaction` - The transaction to save
    ///
    /// # Returns
    /// * `Ok(())` - If the transaction was saved successfully
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn save_transaction(
        &mut self,
        relayer_id: &RelayerId,
        transaction: &Transaction,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans.execute(
                format!("
                INSERT INTO {}(id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, speed, status, expires_at, queued_at, external_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13);
            ", table_name).as_str(),
                &[&transaction.id,
                    &relayer_id,
                    &transaction.to,
                    &transaction.from,
                    &transaction.nonce,
                    &transaction.chain_id,
                    &transaction.data,
                    &transaction.value,
                    &transaction.speed,
                    &transaction.status,
                    &transaction.expires_at,
                    &transaction.queued_at,
                    &transaction.external_id
                ],
            )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    /// Updates a transaction's status to 'Inmempool' after it has been sent to the network.
    ///
    /// Records the transaction hash, gas parameters, and timestamp when the transaction
    /// was successfully sent to the blockchain network.
    ///
    /// # Arguments
    /// * `transaction_id` - The ID of the transaction that was sent
    /// * `transaction_hash` - The hash assigned by the blockchain network
    /// * `sent_with_gas` - The gas price parameters used for sending
    /// * `legacy_transaction` - Whether this is a legacy transaction type
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn transaction_sent(
        &mut self,
        transaction_id: &TransactionId,
        transaction_hash: &TransactionHash,
        sent_with_gas: &GasPriceResult,
        legacy_transaction: bool,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let max_priority_fee_option =
            option_if(!legacy_transaction, &sent_with_gas.max_priority_fee);
        let max_fee_fee_option = option_if(!legacy_transaction, &sent_with_gas.max_fee);
        let legacy_gas_price = option_if(legacy_transaction, sent_with_gas.legacy_gas_price());

        for table_name in TRANSACTION_TABLES.iter() {
            trans
                .execute(
                    format!(
                        "
                            UPDATE {}
                            SET status = $2,
                                hash = $3,
                                sent_max_priority_fee_per_gas = $4,
                                sent_max_fee_per_gas = $5,
                                gas_price = $6,
                                sent_at = NOW()
                            WHERE id = $1;
                        ",
                        table_name
                    )
                    .as_str(),
                    &[
                        &transaction_id,
                        &TransactionStatus::Inmempool,
                        &transaction_hash,
                        &max_priority_fee_option,
                        &max_fee_fee_option,
                        &legacy_gas_price,
                    ],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    /// Records a transaction that failed to be sent to the network.
    ///
    /// Creates a new entry in both transaction tables with a failed status and the reason
    /// for the failure. The failure reason is truncated to 2000 characters.
    ///
    /// # Arguments
    /// * `relayer_id` - The ID of the relayer that attempted to send the transaction
    /// * `transaction` - The transaction that failed to send
    /// * `failed_reason` - The reason why the transaction failed to send
    ///
    /// # Returns
    /// * `Ok(())` - If the failure was recorded successfully
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn transaction_failed_on_send(
        &self,
        relayer_id: &RelayerId,
        transaction: &Transaction,
        failed_reason: &str,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans.execute(
                format!("
                INSERT INTO {}(id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, speed, status, expires_at, queued_at, failed_at, failed_reason, external_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), $13, $14);
                ", table_name).as_str(),
                &[
                    &transaction.id,
                    &relayer_id,
                    &transaction.to,
                    &transaction.from,
                    &transaction.nonce,
                    &transaction.chain_id,
                    &transaction.data,
                    &transaction.value,
                    &transaction.speed,
                    &transaction.status,
                    &transaction.expires_at,
                    &transaction.queued_at,
                    &failed_reason.chars().take(2000).collect::<String>(),
                    &transaction.external_id,
                ],
            )
                .await
                .map_err(PostgresError::PgError)?;
        }

        trans.commit().await.map_err(PostgresError::PgError)?;

        Ok(())
    }

    /// Updates an existing transaction's status to 'Failed'.
    ///
    /// Sets the transaction status to failed, records the failure timestamp and reason.
    /// The failure reason is truncated to 2000 characters.
    ///
    /// # Arguments
    /// * `transaction_id` - The ID of the transaction that failed
    /// * `reason` - The reason why the transaction failed
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn update_transaction_failed(
        &mut self,
        transaction_id: &TransactionId,
        reason: &str,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans
                .execute(
                    format!(
                        "
                            UPDATE {}
                            SET status = $2,
                                failed_at = NOW(),
                                failed_reason = $3
                            WHERE id = $1;
                        ",
                        table_name
                    )
                    .as_str(),
                    &[
                        &transaction_id,
                        &TransactionStatus::Failed,
                        &reason.chars().take(2000).collect::<String>(),
                    ],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    /// Updates a transaction's status to 'Mined' after it has been included in a block.
    ///
    /// Records block information, gas usage, and the timestamp when the transaction
    /// was mined on the blockchain.
    ///
    /// # Arguments
    /// * `transaction_id` - The ID of the transaction that was mined
    /// * `transaction_receipt` - The receipt containing block and gas information
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn transaction_mined(
        &mut self,
        transaction_id: &TransactionId,
        transaction_receipt: &AnyTransactionReceipt,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans
                .execute(
                    format!(
                        "
                            UPDATE {}
                            SET status = $2,
                                gas_limit = $3,
                                block_hash = $4,
                                block_number = $5,
                                mined_at = NOW()
                            WHERE id = $1;
                        ",
                        table_name
                    )
                    .as_str(),
                    &[
                        &transaction_id,
                        &TransactionStatus::Mined,
                        &GasLimit::from(transaction_receipt.gas_used),
                        &transaction_receipt.block_hash.map(|h| BlockHash::new(h)),
                        &transaction_receipt.block_number.map(|n| BlockNumber::new(n)),
                    ],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    /// Updates a transaction's status to 'Confirmed' after sufficient block confirmations.
    ///
    /// Records the timestamp when the transaction reached the required number of
    /// confirmations on the blockchain.
    ///
    /// # Arguments
    /// * `transaction_id` - The ID of the transaction that was confirmed
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn transaction_confirmed(
        &mut self,
        transaction_id: &TransactionId,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans
                .execute(
                    format!(
                        "
                            UPDATE {}
                            SET status = $2,
                                confirmed_at = NOW()
                            WHERE id = $1;
                        ",
                        table_name
                    )
                    .as_str(),
                    &[&transaction_id, &TransactionStatus::Confirmed],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    /// Updates a transaction's status to 'Expired' when it has timed out.
    ///
    /// Records the timestamp when the transaction was marked as expired,
    /// typically when it hasn't been mined within the expected timeframe.
    ///
    /// # Arguments
    /// * `transaction_id` - The ID of the transaction that expired
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn transaction_expired(
        &mut self,
        transaction_id: &TransactionId,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans
                .execute(
                    format!(
                        "
                        UPDATE {}
                        SET status = $2,
                            expired_at = NOW()
                        WHERE id = $1;
                        ",
                        table_name
                    )
                    .as_str(),
                    &[&transaction_id, &TransactionStatus::Expired],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }
}
