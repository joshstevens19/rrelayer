use crate::{
    common_types::EvmAddress,
    gas::{BlobGasPriceResult, GasLimit, GasPriceResult},
    postgres::{PostgresClient, PostgresError},
    relayer::RelayerId,
    shared::{
        common_types::{BlockHash, BlockNumber},
        utils::option_if,
    },
    transaction::types::{
        Transaction, TransactionData, TransactionHash, TransactionId, TransactionNonce,
        TransactionSpeed, TransactionStatus, TransactionValue,
    },
};
use alloy::network::AnyTransactionReceipt;
use serde_json;

const TRANSACTION_TABLES: [&str; 2] = ["relayer.transaction", "relayer.transaction_audit_log"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairedPoisonedPendingTransactions {
    pub count: i64,
    pub min_nonce: Option<TransactionNonce>,
    pub max_nonce: Option<TransactionNonce>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepairedAbsentFutureNoncePendingTransactions {
    pub count: i64,
    pub min_nonce: Option<TransactionNonce>,
    pub max_nonce: Option<TransactionNonce>,
}

impl PostgresClient {
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
                INSERT INTO {}(id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit, speed, status, expires_at, queued_at, hash, external_id, cancelled_by_transaction_id)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17);
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
                    &transaction.external_id,
                    &transaction.cancelled_by_transaction_id
                ],
            )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    /// Persists the exact signed broadcast candidate before its bytes may be
    /// handed to an RPC endpoint. The live transaction row and its audit
    /// snapshot together form the recovery record; no separate attempt table
    /// is required.
    pub async fn transaction_broadcast_attempt(
        &mut self,
        relayer_id: &RelayerId,
        transaction: &Transaction,
        transaction_hash: &TransactionHash,
        sent_with_gas: &GasPriceResult,
        sent_with_blob_gas: Option<&BlobGasPriceResult>,
        legacy_transaction: bool,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let max_priority_fee_option =
            option_if(!legacy_transaction, &sent_with_gas.max_priority_fee);
        let max_fee_option = option_if(!legacy_transaction, &sent_with_gas.max_fee);
        let legacy_gas_price = option_if(legacy_transaction, sent_with_gas.legacy_gas_price());
        let sent_with_gas_json =
            serde_json::to_value(sent_with_gas).unwrap_or(serde_json::Value::Null);
        let sent_with_blob_gas_json = sent_with_blob_gas
            .map(|blob_gas| serde_json::to_value(blob_gas).unwrap_or(serde_json::Value::Null));

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs,
                        gas_limit, speed, status, expires_at, queued_at, hash, external_id,
                        cancelled_by_transaction_id, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, sent_with_gas, sent_with_blob_gas, sent_at
                    )
                    VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                        $16, $17, $18, $19, $20, $21, $22, NOW()
                    )
                    ON CONFLICT (id) DO UPDATE
                    SET relayer_id = EXCLUDED.relayer_id,
                        \"to\" = EXCLUDED.\"to\",
                        \"from\" = EXCLUDED.\"from\",
                        nonce = EXCLUDED.nonce,
                        chain_id = EXCLUDED.chain_id,
                        data = EXCLUDED.data,
                        value = EXCLUDED.value,
                        blobs = EXCLUDED.blobs,
                        gas_limit = EXCLUDED.gas_limit,
                        speed = EXCLUDED.speed,
                        status = EXCLUDED.status,
                        expires_at = EXCLUDED.expires_at,
                        queued_at = EXCLUDED.queued_at,
                        hash = EXCLUDED.hash,
                        external_id = EXCLUDED.external_id,
                        cancelled_by_transaction_id = EXCLUDED.cancelled_by_transaction_id,
                        sent_max_priority_fee_per_gas = EXCLUDED.sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas = EXCLUDED.sent_max_fee_per_gas,
                        gas_price = EXCLUDED.gas_price,
                        sent_with_gas = EXCLUDED.sent_with_gas,
                        sent_with_blob_gas = EXCLUDED.sent_with_blob_gas,
                        sent_at = EXCLUDED.sent_at;
                ",
                &[
                    &transaction.id,
                    relayer_id,
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
                    transaction_hash,
                    &transaction.external_id,
                    &transaction.cancelled_by_transaction_id,
                    &max_priority_fee_option,
                    &max_fee_option,
                    &legacy_gas_price,
                    &sent_with_gas_json,
                    &sent_with_blob_gas_json,
                ],
            )
            .await?;

        trans
            .execute(
                "
                    INSERT INTO relayer.transaction_audit_log (
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs,
                        gas_limit, speed, status, expires_at, queued_at, sent_at, mined_at,
                        confirmed_at, failed_at, failed_reason, hash,
                        sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                        sent_with_gas, sent_with_blob_gas, block_hash, block_number, expired_at,
                        external_id, cancelled_by_transaction_id
                    )
                    SELECT
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs,
                        gas_limit, speed, status, expires_at, queued_at, sent_at, mined_at,
                        confirmed_at, failed_at, failed_reason, hash,
                        sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                        sent_with_gas, sent_with_blob_gas, block_hash, block_number, expired_at,
                        external_id, cancelled_by_transaction_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[&transaction.id],
            )
            .await?;

        trans.commit().await?;
        Ok(())
    }

    pub async fn transaction_sent(
        &mut self,
        transaction_id: &TransactionId,
        transaction_hash: &TransactionHash,
        sent_with_gas: &GasPriceResult,
        sent_with_blob_gas: Option<&BlobGasPriceResult>,
        legacy_transaction: bool,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let max_priority_fee_option =
            option_if(!legacy_transaction, &sent_with_gas.max_priority_fee);
        let max_fee_fee_option = option_if(!legacy_transaction, &sent_with_gas.max_fee);
        let legacy_gas_price = option_if(legacy_transaction, sent_with_gas.legacy_gas_price());

        let sent_with_gas_json =
            serde_json::to_value(sent_with_gas).unwrap_or(serde_json::Value::Null);

        let sent_with_blob_gas_json = sent_with_blob_gas
            .map(|blob_gas| serde_json::to_value(blob_gas).unwrap_or(serde_json::Value::Null));

        trans
            .execute(
                "
                    UPDATE relayer.transaction
                    SET status = $2,
                        hash = $3,
                        sent_max_priority_fee_per_gas = $4,
                        sent_max_fee_per_gas = $5,
                        gas_price = $6,
                        sent_with_gas = $7,
                        sent_with_blob_gas = $8,
                        sent_at = NOW()
                    WHERE id = $1;
                ",
                &[
                    &transaction_id,
                    &TransactionStatus::INMEMPOOL,
                    &transaction_hash,
                    &max_priority_fee_option,
                    &max_fee_fee_option,
                    &legacy_gas_price,
                    &sent_with_gas_json,
                    &sent_with_blob_gas_json,
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
                        sent_max_fee_per_gas, gas_price, sent_with_gas, sent_with_blob_gas, external_id
                    )
                    SELECT
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, $2, expires_at, queued_at, NOW(), mined_at, confirmed_at,
                        failed_at, failed_reason, $3, $4, $5, $6, $7, $8, external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[
                    &transaction_id,
                    &TransactionStatus::INMEMPOOL,
                    &transaction_hash,
                    &max_priority_fee_option,
                    &max_fee_fee_option,
                    &legacy_gas_price,
                    &sent_with_gas_json,
                    &sent_with_blob_gas_json,
                ],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

    /// Advances a landed, previously recorded attempt from PENDING to INMEMPOOL.
    /// The guarded update and audit snapshot commit before startup loads any
    /// in-memory queue state.
    pub async fn recover_landed_transaction_attempt(
        &self,
        relayer_id: &RelayerId,
        transaction_id: &TransactionId,
        transaction_hash: &TransactionHash,
        sent_with_gas: &GasPriceResult,
        sent_with_blob_gas: Option<&BlobGasPriceResult>,
        legacy_transaction: bool,
    ) -> Result<bool, PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let max_priority_fee_option =
            option_if(!legacy_transaction, &sent_with_gas.max_priority_fee);
        let max_fee_option = option_if(!legacy_transaction, &sent_with_gas.max_fee);
        let legacy_gas_price = option_if(legacy_transaction, sent_with_gas.legacy_gas_price());
        let sent_with_gas_json =
            serde_json::to_value(sent_with_gas).unwrap_or(serde_json::Value::Null);
        let sent_with_blob_gas_json = sent_with_blob_gas
            .map(|blob_gas| serde_json::to_value(blob_gas).unwrap_or(serde_json::Value::Null));

        let row = trans
            .query_one(
                "
                    WITH recovered AS (
                        UPDATE relayer.transaction
                        SET status = $4,
                            hash = $3,
                            sent_max_priority_fee_per_gas = $5,
                            sent_max_fee_per_gas = $6,
                            gas_price = $7,
                            sent_with_gas = $8,
                            sent_with_blob_gas = $9,
                            sent_at = COALESCE(sent_at, NOW())
                        WHERE id = $1
                          AND relayer_id = $2
                          AND status = $10
                          AND failed_at IS NULL
                          AND EXISTS (
                              SELECT 1
                              FROM relayer.transaction_audit_log attempt
                              WHERE attempt.id = $1
                                AND attempt.hash = $3
                                AND attempt.sent_at IS NOT NULL
                          )
                        RETURNING *
                    ), audit AS (
                        INSERT INTO relayer.transaction_audit_log (
                            id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs,
                            gas_limit, speed, status, expires_at, queued_at, sent_at, mined_at,
                            confirmed_at, failed_at, failed_reason, hash,
                            sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                            sent_with_gas, sent_with_blob_gas, block_hash, block_number, expired_at,
                            external_id, cancelled_by_transaction_id
                        )
                        SELECT
                            id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs,
                            gas_limit, speed, status, expires_at, queued_at, sent_at, mined_at,
                            confirmed_at, failed_at, failed_reason, hash,
                            sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                            sent_with_gas, sent_with_blob_gas, block_hash, block_number, expired_at,
                            external_id, cancelled_by_transaction_id
                        FROM recovered
                        RETURNING id
                    )
                    SELECT COUNT(*)::BIGINT AS recovered_count FROM audit;
                ",
                &[
                    transaction_id,
                    relayer_id,
                    transaction_hash,
                    &TransactionStatus::INMEMPOOL,
                    &max_priority_fee_option,
                    &max_fee_option,
                    &legacy_gas_price,
                    &sent_with_gas_json,
                    &sent_with_blob_gas_json,
                    &TransactionStatus::PENDING,
                ],
            )
            .await
            .map_err(PostgresError::PgError)?;

        trans.commit().await.map_err(PostgresError::PgError)?;
        Ok(row.get::<_, i64>("recovered_count") == 1)
    }

    pub async fn repair_poisoned_pending_transactions_for_relayer(
        &self,
        relayer_id: &RelayerId,
    ) -> Result<RepairedPoisonedPendingTransactions, PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let row = trans
            .query_one(
                "
                    WITH repaired AS (
                        UPDATE relayer.transaction
                        SET status = $2,
                            failed_reason = COALESCE(failed_reason, $4)
                        WHERE relayer_id = $1
                          AND status = $3
                          AND failed_at IS NOT NULL
                          AND hash IS NULL
                          AND sent_at IS NULL
                        RETURNING *
                    ), audit AS (
                        INSERT INTO relayer.transaction_audit_log (
                            id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs,
                            gas_limit, speed, status, expires_at, queued_at, sent_at, mined_at,
                            confirmed_at, failed_at, failed_reason, hash,
                            sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                            sent_with_gas, sent_with_blob_gas, block_hash, block_number, expired_at,
                            external_id, cancelled_by_transaction_id
                        )
                        SELECT
                            id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs,
                            gas_limit, speed, status, expires_at, queued_at, sent_at, mined_at,
                            confirmed_at, failed_at, failed_reason, hash,
                            sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                            sent_with_gas, sent_with_blob_gas, block_hash, block_number, expired_at,
                            external_id, cancelled_by_transaction_id
                        FROM repaired
                        RETURNING nonce
                    )
                    SELECT COUNT(*)::BIGINT AS repaired_count,
                           MIN(nonce)::BIGINT AS min_nonce,
                           MAX(nonce)::BIGINT AS max_nonce
                    FROM audit;
                ",
                &[
                    relayer_id,
                    &TransactionStatus::FAILED,
                    &TransactionStatus::PENDING,
                    &"startup repair: terminalized unsent failed pending transaction",
                ],
            )
            .await
            .map_err(PostgresError::PgError)?;

        trans.commit().await.map_err(PostgresError::PgError)?;

        Ok(RepairedPoisonedPendingTransactions {
            count: row.get("repaired_count"),
            min_nonce: row.get::<_, Option<i64>>("min_nonce").map(TransactionNonce::from),
            max_nonce: row.get::<_, Option<i64>>("max_nonce").map(TransactionNonce::from),
        })
    }

    pub async fn repair_absent_future_nonce_pending_transactions_for_relayer(
        &self,
        relayer_id: &RelayerId,
        chain_nonce: &TransactionNonce,
        checked_absent_transactions: &[(TransactionId, Vec<TransactionHash>)],
        failed_reason: &str,
    ) -> Result<RepairedAbsentFutureNoncePendingTransactions, PostgresError> {
        if checked_absent_transactions.is_empty() {
            return Ok(RepairedAbsentFutureNoncePendingTransactions {
                count: 0,
                min_nonce: None,
                max_nonce: None,
            });
        }

        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let mut count = 0;
        let mut min_nonce: Option<TransactionNonce> = None;
        let mut max_nonce: Option<TransactionNonce> = None;

        for (transaction_id, transaction_hashes) in checked_absent_transactions {
            let row = trans
                .query_one(
                    "
                        WITH repaired AS (
                            UPDATE relayer.transaction
                            SET status = $3,
                                failed_at = NOW(),
                                failed_reason = COALESCE(failed_reason, $5)
                            WHERE id = $1
                              AND relayer_id = $2
                              AND status = $4
                              AND sent_at IS NOT NULL
                              AND failed_at IS NULL
                              AND hash IS NOT NULL
                              AND hash = ANY($7::BYTEA[])
                              AND NOT EXISTS (
                                  SELECT 1
                                  FROM relayer.transaction_audit_log attempts
                                  WHERE attempts.id = $1
                                    AND attempts.sent_at IS NOT NULL
                                    AND attempts.hash IS NOT NULL
                                    AND NOT (attempts.hash = ANY($7::BYTEA[]))
                              )
                              AND nonce > $6
                            RETURNING *
                        ), audit AS (
                            INSERT INTO relayer.transaction_audit_log (
                                id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value,
                                blobs, gas_limit, speed, status, expires_at, queued_at, sent_at,
                                mined_at, confirmed_at, failed_at, failed_reason, hash,
                                sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                                sent_with_gas, sent_with_blob_gas, block_hash, block_number,
                                expired_at, external_id, cancelled_by_transaction_id
                            )
                            SELECT
                                id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value,
                                blobs, gas_limit, speed, status, expires_at, queued_at, sent_at,
                                mined_at, confirmed_at, failed_at, failed_reason, hash,
                                sent_max_priority_fee_per_gas, sent_max_fee_per_gas, gas_price,
                                sent_with_gas, sent_with_blob_gas, block_hash, block_number,
                                expired_at, external_id, cancelled_by_transaction_id
                            FROM repaired
                            RETURNING nonce
                        )
                        SELECT COUNT(*)::BIGINT AS repaired_count,
                               MIN(nonce)::BIGINT AS min_nonce,
                               MAX(nonce)::BIGINT AS max_nonce
                        FROM audit;
                    ",
                    &[
                        transaction_id,
                        relayer_id,
                        &TransactionStatus::FAILED,
                        &TransactionStatus::PENDING,
                        &failed_reason,
                        chain_nonce,
                        transaction_hashes,
                    ],
                )
                .await
                .map_err(PostgresError::PgError)?;

            let repaired_count: i64 = row.get("repaired_count");
            count += repaired_count;

            if let Some(nonce) = row.get::<_, Option<i64>>("min_nonce").map(TransactionNonce::from)
            {
                min_nonce = Some(match min_nonce {
                    Some(current) if current.into_inner() <= nonce.into_inner() => current,
                    _ => nonce,
                });
            }
            if let Some(nonce) = row.get::<_, Option<i64>>("max_nonce").map(TransactionNonce::from)
            {
                max_nonce = Some(match max_nonce {
                    Some(current) if current.into_inner() >= nonce.into_inner() => current,
                    _ => nonce,
                });
            }
        }

        trans.commit().await.map_err(PostgresError::PgError)?;

        Ok(RepairedAbsentFutureNoncePendingTransactions { count, min_nonce, max_nonce })
    }

    pub async fn transaction_failed_on_send(
        &self,
        relayer_id: &RelayerId,
        transaction: &Transaction,
        failed_reason: String,
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
                    &transaction.value,
                    &transaction.blobs,
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

    pub async fn update_transaction_noop(
        &mut self,
        transaction_id: &TransactionId,
        to: &EvmAddress,
        speed: &TransactionSpeed,
        gas_limit: &GasLimit,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        trans
            .execute(
                "
                    UPDATE relayer.transaction
                    SET \"to\" = $2,
                        value = $3,
                        data = $4,
                        blobs = NULL,
                        gas_limit = $5,
                        speed = $6
                    WHERE id = $1;
                ",
                &[
                    &transaction_id,
                    &to,
                    &TransactionValue::zero(),
                    &TransactionData::empty(),
                    gas_limit,
                    speed,
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
                        sent_max_fee_per_gas, gas_price, external_id
                    )
                    SELECT
                        id, relayer_id, $2, \"from\", nonce, chain_id, $4, $3, NULL, $5,
                        $6, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[
                    &transaction_id,
                    &to,
                    &TransactionValue::zero(),
                    &TransactionData::empty(),
                    gas_limit,
                    speed,
                ],
            )
            .await
            .map_err(PostgresError::PgError)?;

        trans.commit().await.map_err(PostgresError::PgError)?;

        Ok(())
    }

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
                &[&transaction_id, &TransactionStatus::FAILED, &truncated_reason],
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
                    &TransactionStatus::FAILED,
                    &truncated_reason,
                ],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

    pub async fn transaction_mined(
        &mut self,
        transaction: &Transaction,
        transaction_receipt: &AnyTransactionReceipt,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let gas_used = GasLimit::from(transaction_receipt.gas_used);
        let block_hash = transaction_receipt.block_hash.map(BlockHash::new);
        let block_number = transaction_receipt.block_number.map(BlockNumber::new);
        let hash = TransactionHash::new(transaction_receipt.transaction_hash);

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
                    &TransactionStatus::MINED,
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
                    &hash,
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
                    &TransactionStatus::MINED,
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
                &[&transaction_id, &TransactionStatus::CONFIRMED],
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
                &[&transaction_id, &TransactionStatus::CONFIRMED],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

    pub async fn transaction_update_nonce(
        &mut self,
        transaction_id: &TransactionId,
        nonce: &TransactionNonce,
    ) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        trans
            .execute(
                "UPDATE relayer.transaction SET nonce = $2 WHERE id = $1",
                &[&transaction_id, &(nonce.into_inner() as i64)],
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
                        id, relayer_id, \"to\", \"from\", $2, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, block_hash, block_number, external_id
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[&transaction_id, &(nonce.into_inner() as i64)],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

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
                &[&transaction_id, &TransactionStatus::EXPIRED],
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
                &[&transaction_id, &TransactionStatus::EXPIRED],
            )
            .await?;

        trans.commit().await?;

        Ok(())
    }

    pub async fn transaction_update(&self, transaction: &Transaction) -> Result<(), PostgresError> {
        let mut conn = self.pool.get().await?;
        let trans = conn.transaction().await.map_err(PostgresError::PgError)?;

        let sent_with_gas_json = transaction
            .sent_with_gas
            .as_ref()
            .map(|gas| serde_json::to_value(gas).unwrap_or(serde_json::Value::Null));

        let sent_with_blob_gas_json = transaction
            .sent_with_blob_gas
            .as_ref()
            .map(|blob_gas| serde_json::to_value(blob_gas).unwrap_or(serde_json::Value::Null));

        let truncated_failed_reason = transaction
            .failed_reason
            .as_ref()
            .map(|reason| reason.chars().take(2000).collect::<String>());

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
                        sent_with_gas = $20,
                        sent_with_blob_gas = $21,
                        external_id = $22,
                        cancelled_by_transaction_id = $23,
                        blobs = $24,
                        failed_reason = $25,
                        failed_at = CASE WHEN $25::TEXT IS NULL THEN failed_at ELSE NOW() END
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
                    &sent_with_gas_json,
                    &sent_with_blob_gas_json,
                    &transaction.external_id,
                    &transaction.cancelled_by_transaction_id,
                    &transaction.blobs,
                    &truncated_failed_reason,
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
                        sent_max_fee_per_gas, gas_price, sent_with_gas, sent_with_blob_gas,
                        block_hash, block_number, expired_at, external_id
                    )
                    SELECT
                        id, relayer_id, \"to\", \"from\", nonce, chain_id, data, value, blobs, gas_limit,
                        speed, status, expires_at, queued_at, sent_at, mined_at, confirmed_at,
                        failed_at, failed_reason, hash, sent_max_priority_fee_per_gas,
                        sent_max_fee_per_gas, gas_price, sent_with_gas, sent_with_blob_gas,
                        block_hash, block_number, expired_at, external_id
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
