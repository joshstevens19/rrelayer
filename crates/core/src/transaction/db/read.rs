use super::{builders::build_transaction_from_transaction_view, RecordedTransactionAttempt};
use crate::{
    postgres::{PostgresClient, PostgresError},
    relayer::RelayerId,
    shared::common_types::{PagingContext, PagingResult},
    transaction::types::{Transaction, TransactionHash, TransactionId, TransactionStatus},
};

impl PostgresClient {
    /// Loads unique broadcast attempts in durable persistence order.
    ///
    /// Audit history supplies the stable sequence. The live row is a fallback
    /// for installations that predate attempt snapshots or were interrupted
    /// between the live-row update and an older audit-writing path.
    pub(crate) async fn get_recorded_transaction_attempts(
        &self,
        id: &TransactionId,
    ) -> Result<Vec<RecordedTransactionAttempt>, PostgresError> {
        let rows = self
            .query(
                "
                    WITH raw_attempts AS (
                        SELECT
                            hash,
                            sent_with_gas,
                            sent_with_blob_gas,
                            sent_at,
                            NULL::BIGINT AS history_id
                        FROM relayer.transaction
                        WHERE id = $1

                        UNION ALL

                        SELECT
                            hash,
                            sent_with_gas,
                            sent_with_blob_gas,
                            sent_at,
                            history_id::BIGINT
                        FROM relayer.transaction_audit_log
                        WHERE id = $1
                    ), unique_attempts AS (
                        SELECT DISTINCT ON (hash)
                            hash,
                            sent_with_gas,
                            sent_with_blob_gas,
                            sent_at,
                            history_id
                        FROM raw_attempts
                        WHERE hash IS NOT NULL
                          AND sent_at IS NOT NULL
                        ORDER BY hash, history_id ASC NULLS LAST
                    )
                    SELECT hash, sent_with_gas, sent_with_blob_gas
                    FROM unique_attempts
                    ORDER BY history_id ASC NULLS LAST, sent_at ASC, hash ASC;
                ",
                &[id],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|row| RecordedTransactionAttempt {
                hash: row.get("hash"),
                sent_with_gas: row
                    .get::<_, Option<serde_json::Value>>("sent_with_gas")
                    .and_then(|value| serde_json::from_value(value).ok()),
                sent_with_blob_gas: row
                    .get::<_, Option<serde_json::Value>>("sent_with_blob_gas")
                    .and_then(|value| serde_json::from_value(value).ok()),
            })
            .collect())
    }

    pub async fn get_transaction(
        &self,
        id: &TransactionId,
    ) -> Result<Option<Transaction>, PostgresError> {
        let row = self
            .query_one_or_none(
                "
                    SELECT *
                    FROM relayer.transaction
                    WHERE id = $1;
                ",
                &[id],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(row) => Ok(Some(build_transaction_from_transaction_view(&row))),
        }
    }

    pub async fn get_transactions_for_relayer(
        &self,
        id: &RelayerId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<Transaction>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer.transaction
                    WHERE relayer_id = $1
                    LIMIT $2
                    OFFSET $3;
                ",
                &[&id, &(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<Transaction> =
            rows.iter().map(build_transaction_from_transaction_view).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn get_transactions_by_status_for_relayer(
        &self,
        id: &RelayerId,
        status: &TransactionStatus,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<Transaction>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer.transaction
                    WHERE relayer_id = $1
                    AND status = $2
                    ORDER BY nonce ASC
                    LIMIT $3
                    OFFSET $4;
                ",
                &[id, status, &(paging_context.limit as i64), &(paging_context.offset as i64)],
            )
            .await?;

        let results: Vec<Transaction> =
            rows.iter().map(build_transaction_from_transaction_view).collect();

        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn get_pending_transactions_with_attempt_evidence_for_relayer(
        &self,
        id: &RelayerId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<Transaction>, PostgresError> {
        let rows = self
            .query(
                "
                    SELECT *
                    FROM relayer.transaction
                    WHERE relayer_id = $1
                      AND status = $2
                      AND sent_at IS NOT NULL
                      AND failed_at IS NULL
                      AND hash IS NOT NULL
                    ORDER BY nonce ASC
                    LIMIT $3
                    OFFSET $4;
                ",
                &[
                    id,
                    &TransactionStatus::PENDING,
                    &(paging_context.limit as i64),
                    &(paging_context.offset as i64),
                ],
            )
            .await?;

        let results: Vec<Transaction> =
            rows.iter().map(build_transaction_from_transaction_view).collect();
        let result_count = results.len();

        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    pub async fn get_transaction_by_hash(
        &self,
        hash: &TransactionHash,
    ) -> Result<Option<Transaction>, PostgresError> {
        let row = self
            .query_one_or_none(
                "
                    SELECT *
                    FROM relayer.transaction
                    WHERE hash = $1;
                ",
                &[hash],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(row) => Ok(Some(build_transaction_from_transaction_view(&row))),
        }
    }

    pub async fn get_transaction_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<Transaction>, PostgresError> {
        let row = self
            .query_one_or_none(
                "
                    SELECT *
                    FROM relayer.transaction
                    WHERE external_id = $1;
                ",
                &[&external_id],
            )
            .await?;

        match row {
            None => Ok(None),
            Some(row) => Ok(Some(build_transaction_from_transaction_view(&row))),
        }
    }
}
