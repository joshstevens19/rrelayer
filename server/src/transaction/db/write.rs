use alloy::rpc::types::TransactionReceipt;

use crate::{
    gas::{fee_estimator::base::GasPriceResult, types::GasLimit},
    postgres::PostgresClient,
    relayer::types::RelayerId,
    shared::{
        common_types::{BlockHash, BlockNumber},
        utils::option_if,
    },
    transaction::types::{Transaction, TransactionHash, TransactionId, TransactionStatus},
};

const TRANSACTION_TABLES: [&str; 2] = ["relayer_transaction", "relayer_transaction_audit_log"];

impl PostgresClient {
    pub async fn save_transaction(
        &mut self,
        relayer_id: &RelayerId,
        transaction: &Transaction,
    ) -> Result<(), tokio_postgres::Error> {
        let trans: tokio_postgres::Transaction = self.transaction().await?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans.execute(
            format!("
                INSERT INTO {}(id, relayer_id, api_key, \"to\", \"from\", nonce, chain_id, data, value, speed, status, expiries_at, queued_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13);
            ", table_name).as_str(),
            &[&transaction.id,
              &relayer_id,
              &transaction.from_api_key,
              &transaction.to.hex(),
              &transaction.from.hex(),
              &transaction.nonce,
              &transaction.chain_id,
              &transaction.data,
              &transaction.value,
              &transaction.speed.to_string(),
              &transaction.status.to_string(),
              &transaction.expires_at,
              &transaction.queued_at],
        )
        .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    pub async fn transaction_sent(
        &mut self,
        transaction_id: &TransactionId,
        transaction_hash: &TransactionHash,
        sent_with_gas: &GasPriceResult,
        legacy_transaction: bool,
    ) -> Result<(), tokio_postgres::Error> {
        let trans: tokio_postgres::Transaction = self.transaction().await?;

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
                        &TransactionStatus::Inmempool.to_string(),
                        &transaction_hash.hex(),
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

    pub async fn transaction_failed_on_send(
        &mut self,
        relayer_id: &RelayerId,
        transaction: &Transaction,
        failed_reason: &str,
    ) -> Result<(), tokio_postgres::Error> {
        let trans: tokio_postgres::Transaction = self.transaction().await?;

        for table_name in TRANSACTION_TABLES.iter() {
            trans.execute(
                format!("
                INSERT INTO {}(id, relayer_id, api_key, \"to\", nonce, chain_id, data, value, speed, status, expiries_at, queued_at, failed_at, failed_reason)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW());
                ", table_name).as_str(),
                &[&transaction.id,
                &relayer_id,
                &transaction.from_api_key,
                &transaction.to.hex(),
                &transaction.nonce,
                &transaction.chain_id,
                &transaction.data,
                &transaction.value,
                &transaction.speed.to_string(),
                &transaction.status.to_string(),
                &transaction.expires_at,
                &transaction.queued_at,
                &failed_reason.chars().take(2000).collect::<String>()],
            )
            .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    pub async fn update_transaction_failed(
        &mut self,
        transaction_id: &TransactionId,
        reason: &str,
    ) -> Result<(), tokio_postgres::Error> {
        let trans: tokio_postgres::Transaction = self.transaction().await?;

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
                        &TransactionStatus::Failed.to_string(),
                        &reason.chars().take(2000).collect::<String>(),
                    ],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    pub async fn transaction_mined(
        &mut self,
        transaction_id: &TransactionId,
        transaction_receipt: &TransactionReceipt,
    ) -> Result<(), tokio_postgres::Error> {
        let trans: tokio_postgres::Transaction = self.transaction().await?;

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
                        &TransactionStatus::Mined.to_string(),
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

    pub async fn transaction_confirmed(
        &mut self,
        transaction_id: &TransactionId,
    ) -> Result<(), tokio_postgres::Error> {
        let trans: tokio_postgres::Transaction = self.transaction().await?;

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
                    &[&transaction_id, &TransactionStatus::Confirmed.to_string()],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }

    pub async fn transaction_expired(
        &mut self,
        transaction_id: &TransactionId,
    ) -> Result<(), tokio_postgres::Error> {
        let trans: tokio_postgres::Transaction = self.transaction().await?;

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
                    &[&transaction_id, &TransactionStatus::Expired.to_string()],
                )
                .await?;
        }

        trans.commit().await?;

        Ok(())
    }
}
