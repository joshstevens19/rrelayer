use crate::{
    common_types::EvmAddress,
    gas::{fee_estimator::base::GasPriceResult, types::GasLimit},
    postgres::{PostgresClient, PostgresError},
    relayer::RelayerId,
    shared::{
        common_types::{BlockHash, BlockNumber},
        utils::option_if,
    },
    transaction::types::{
        Transaction, TransactionData, TransactionHash, TransactionId, TransactionStatus,
        TransactionValue,
    },
};
use alloy::network::AnyTransactionReceipt;

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
                INSERT INTO {}(id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit, speed, status, expires_at, queued_at, hash, external_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16);
            ", table_name).as_str(),
                &[&transaction.id,
                    &relayer_id,
                    &transaction.to,
                    &transaction.from,
                    &transaction.nonce,
                    &transaction.chain_id,
                    &transaction.data,
                    &transaction.value,
                    &transaction.blobs,
                    &transaction.gas_limit,
                    &transaction.speed,
                    &transaction.status,
                    &transaction.expires_at,
                    &transaction.queued_at,
                    &transaction.known_transaction_hash,
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
    /// Updates the main transaction table and creates a new audit log entry.
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

        trans
            .execute(
                "
                    UPDATE relayer.transaction
                    SET status = $2,
                        hash = $3,
                        sent_max_priority_fee_per_gas = $4,
                        sent_max_fee_per_gas = $5,
                        gas_price = $6,
                        sent_at = NOW()
                    WHERE id = $1;
                ",
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

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction_audit_log (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit, 
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at, 
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas, 
                        sent_max_fee_per_gas, gas_price, external_id
                    )
                    SELECT 
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, $2, expires_at, queued_at, NOW(), mined_at, confirmed_at,
                        failed_at, failed_reason, $3, $4, $5, $6, external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
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
                INSERT INTO {}(id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, speed, status, expires_at, queued_at, failed_at, failed_reason, external_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW(), $14, $15);
                ", table_name).as_str(),
                &[
                    &transaction.id,
                    &relayer_id,
                    &transaction.to,
                    &transaction.from,
                    &transaction.nonce,
                    &transaction.chain_id,
                    &transaction.data,
                    &transaction.blobs,
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

    /// Updates a transaction's content when converted to a no-op during cancellation.
    /// Updates the main transaction table and creates a new audit log entry.
    ///
    /// # Arguments
    /// * `transaction_id` - The unique identifier of the transaction
    /// * `to` - The new recipient address (usually the relayer's own address)
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn update_transaction_noop(
        &mut self,
        transaction_id: &TransactionId,
        to: &EvmAddress,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        trans
            .execute(
                "
                    UPDATE relayer.transaction
                    SET \"to\" = $2,
                        value = $3,
                        data = $4
                    WHERE id = $1;
                ",
                &[&transaction_id, &to, &TransactionValue::zero(), &TransactionData::empty()],
            )
            .await
            .map_err(PostgresError::PgError)?;

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction_audit_log (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, external_id
                    )
                    SELECT 
                        id, relayer_id, $2, \"from\", nonce, chain_id, $4, $3, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[&transaction_id, &to, &TransactionValue::zero(), &TransactionData::empty()],
            )
            .await
            .map_err(PostgresError::PgError)?;

        trans.commit().await.map_err(PostgresError::PgError)?;

        Ok(())
    }

    /// Updates an existing transaction's status to 'Failed'.
    /// Updates the main transaction table and creates a new audit log entry.
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

        let truncated_reason = reason.chars().take(2000).collect::<String>();

        trans
            .execute(
                "
                    UPDATE relayer.transaction
                    SET status = $2,
                        failed_at = NOW(),
                        failed_reason = $3
                    WHERE id = $1;
                ",
                &[&transaction_id, &TransactionStatus::Failed, &truncated_reason],
            )
            .await?;

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction_audit_log (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, external_id
                    )
                    SELECT 
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, $2, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        NOW(), $3, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[
                    &transaction_id,
                    &TransactionStatus::Failed,
                    &truncated_reason,
                ],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

    /// Updates a transaction's status to 'Mined' after it has been included in a block.
    /// Updates the main transaction table and creates a new audit log entry.
    ///
    /// Records block information, gas usage, and the timestamp when the transaction
    /// was mined on the blockchain.
    ///
    /// # Arguments
    /// * `transaction` - The transaction that was mined
    /// * `transaction_receipt` - The receipt containing block and gas information
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn transaction_mined(
        &mut self,
        transaction: &Transaction,
        transaction_receipt: &AnyTransactionReceipt,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let gas_used = GasLimit::from(transaction_receipt.gas_used);
        let block_hash = transaction_receipt.block_hash.map(|h| BlockHash::new(h));
        let block_number = transaction_receipt.block_number.map(|n| BlockNumber::new(n));

        trans
            .execute(
                "
                UPDATE relayer.transaction
                SET status = $2,
                    \"to\" = $3,
                    \"from\" = $4,
                    value = $5,
                    data = $6,
                    nonce = $7,
                    chain_id = $8,
                    gas_limit = $9,
                    block_hash = $10,
                    block_number = $11,
                    speed = $12,
                    hash = $13,
                    sent_max_fee_per_gas = $14,
                    sent_max_priority_fee_per_gas = $15,
                    external_id = $16,
                    mined_at = NOW()
                WHERE id = $1;
            ",
                &[
                    &transaction.id,
                    &TransactionStatus::Mined,
                    &transaction.to,
                    &transaction.from,
                    &transaction.value,
                    &transaction.data,
                    &transaction.nonce,
                    &transaction.chain_id,
                    &gas_used,
                    &block_hash,
                    &block_number,
                    &transaction.speed,
                    &transaction.known_transaction_hash,
                    &transaction.sent_with_max_fee_per_gas,
                    &transaction.sent_with_max_priority_fee_per_gas,
                    &transaction.external_id,
                ],
            )
            .await?;

        trans
            .execute(
                "
                INSERT INTO relayer.transaction_audit_log (
                    id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                    speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                    failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                    sent_max_fee_per_gas, gas_price, block_hash, block_number, external_id
                )
                SELECT 
                    $1, relayer_id, $3, $4, $7, $8, $6, $5, blobs, $9,
                    $12, $2, expires_at, queued_at, sent_at, NOW(), confirmed_at,
                    failed_at, failed_reason, $13, $15, $14, gas_price, $10, $11, $16
                FROM relayer.transaction
                WHERE id = $1;
            ",
                &[
                    &transaction.id,
                    &TransactionStatus::Mined,
                    &transaction.to,
                    &transaction.from,
                    &transaction.value,
                    &transaction.data,
                    &transaction.nonce,
                    &transaction.chain_id,
                    &gas_used,
                    &block_hash,
                    &block_number,
                    &transaction.speed,
                    &transaction.known_transaction_hash,
                    &transaction.sent_with_max_fee_per_gas,
                    &transaction.sent_with_max_priority_fee_per_gas,
                    &transaction.external_id,
                ],
            )
            .await?;

        trans.commit().await?;
        Ok(())
    }

    /// Updates a transaction's status to 'Confirmed' after sufficient block confirmations.
    /// Updates the main transaction table and creates a new audit log entry.
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

        trans
            .execute(
                "
                    UPDATE relayer.transaction
                    SET status = $2,
                        confirmed_at = NOW()
                    WHERE id = $1;
                ",
                &[&transaction_id, &TransactionStatus::Confirmed],
            )
            .await?;

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction_audit_log (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, block_hash, block_number, external_id
                    )
                    SELECT 
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, $2, expires_at, queued_at, sent_at, mined_at, NOW(),
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, block_hash, block_number, external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[&transaction_id, &TransactionStatus::Confirmed],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

    /// Updates a transaction's status to 'Expired' when it has timed out.
    /// Updates the main transaction table and creates a new audit log entry.
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

        trans
            .execute(
                "
                UPDATE relayer.transaction
                SET status = $2,
                    expired_at = NOW()
                WHERE id = $1;
                ",
                &[&transaction_id, &TransactionStatus::Expired],
            )
            .await?;

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction_audit_log (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, block_hash, block_number, expired_at, external_id
                    )
                    SELECT 
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, $2, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, block_hash, block_number, NOW(), external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[&transaction_id, &TransactionStatus::Expired],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

    /// Updates an existing transaction with new data.
    /// Updates the main transaction table and creates a new audit log entry.
    ///
    /// This is useful for recording changes like replacements, gas bumps, or other modifications.
    ///
    /// # Arguments
    /// * `transaction` - The transaction with updated data
    ///
    /// # Returns
    /// * `Ok(())` - If the update was successful
    /// * `Err(PostgresError)` - If a database error occurs
    pub async fn transaction_update(&self, transaction: &Transaction) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        trans
            .execute(
                "
                    UPDATE relayer.transaction
                    SET relayer_id = $2,
                        \"to\" = $3,
                        \"from\" = $4,
                        nonce = $5,
                        chain_id = $6,
                        data = $7,
                        value = $8,
                        speed = $9,
                        status = $10,
                        expires_at = $11,
                        queued_at = $12,
                        sent_at = $13,
                        mined_at = $14,
                        confirmed_at = $15,
                        gas_limit = $16,
                        hash = $17,
                        sent_max_fee_per_gas = $18,
                        sent_max_priority_fee_per_gas = $19,
                        external_id = $20
                    WHERE id = $1
                ",
                &[
                    &transaction.id,
                    &transaction.relayer_id,
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
                    &transaction.sent_at,
                    &transaction.mined_at,
                    &transaction.confirmed_at,
                    &transaction.gas_limit,
                    &transaction.known_transaction_hash,
                    &transaction.sent_with_max_fee_per_gas,
                    &transaction.sent_with_max_priority_fee_per_gas,
                    &transaction.external_id,
                ],
            )
            .await
            .map_err(PostgresError::PgError)?;

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction_audit_log (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, block_hash, block_number, expired_at, external_id
                    )
                    SELECT 
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, block_hash, block_number, expired_at, external_id
                    FROM relayer.transaction
                    WHERE id = $1
                ",
                &[&transaction.id],
            )
            .await
            .map_err(PostgresError::PgError)?;

        trans.commit().await.map_err(PostgresError::PgError)?;
        Ok(())
    }
}
