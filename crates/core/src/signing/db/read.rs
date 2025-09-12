use crate::common_types::{EvmAddress, Signature};
use crate::network::types::ChainId;
use crate::{
    postgres::{PostgresClient, PostgresError},
    relayer::types::RelayerId,
    shared::common_types::{PagingContext, PagingResult},
};
use chrono::{DateTime, Utc};
use google_secretmanager1::client::serde_with::serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedTextHistory {
    pub relayer_id: RelayerId,
    pub message: String,
    pub signature: Signature,
    pub chain_id: ChainId,
    pub signed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedTypedDataHistory {
    pub relayer_id: RelayerId,
    pub domain_data: serde_json::Value,
    pub message_data: serde_json::Value,
    pub primary_type: String,
    pub signature: Signature,
    pub chain_id: ChainId,
    pub signed_at: DateTime<Utc>,
}

impl PostgresClient {
    /// Retrieves signed text message history with pagination for a specific relayer.
    ///
    /// # Arguments
    /// * `relayer_id` - The relayer ID to filter by
    /// * `paging_context` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    /// * `Ok(PagingResult<SignedTextHistory>)` - Paginated list of signed text messages
    /// * `Err(PostgresError)` - If the database operation fails
    pub async fn get_signed_text_history(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<SignedTextHistory>, PostgresError> {
        let conn = self.pool.get().await?;
        let rows = conn
            .query(
                "
                    SELECT id, relayer_id, message, signature, chain_id, signed_at
                    FROM signing.text_history
                    WHERE relayer_id = $1
                    ORDER BY signed_at DESC
                    LIMIT $2
                    OFFSET $3;
                ",
                &[
                    &relayer_id,
                    &(paging_context.limit as i64),
                    &(paging_context.offset as i64)
                ],
            )
            .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(SignedTextHistory {
                relayer_id: row.get("relayer_id"),
                message: row.get("message"),
                signature: row.get("signature"),
                chain_id: row.get("chain_id"),
                signed_at: row.get::<_, DateTime<Utc>>("signed_at"),
            });
        }

        let result_count = results.len();
        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }

    /// Retrieves signed typed data message history with pagination for a specific relayer.
    ///
    /// # Arguments
    /// * `relayer_id` - The relayer ID to filter by
    /// * `paging_context` - Pagination parameters (limit and offset)
    ///
    /// # Returns
    /// * `Ok(PagingResult<SignedTypedDataHistory>)` - Paginated list of signed typed data messages
    /// * `Err(PostgresError)` - If the database operation fails
    pub async fn get_signed_typed_data_history(
        &self,
        relayer_id: &RelayerId,
        paging_context: &PagingContext,
    ) -> Result<PagingResult<SignedTypedDataHistory>, PostgresError> {
        let conn = self.pool.get().await?;
        let rows = conn
            .query(
                "
                    SELECT id, relayer_id, domain_data, message_data, primary_type, signature, chain_id, signed_at
                    FROM signing.typed_data_history
                    WHERE relayer_id = $1
                    ORDER BY signed_at DESC
                    LIMIT $2
                    OFFSET $3;
                ",
                &[
                    &relayer_id,
                    &(paging_context.limit as i64),
                    &(paging_context.offset as i64)
                ],
            )
            .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(SignedTypedDataHistory {
                relayer_id: row.get("relayer_id"),
                domain_data: row.get("domain_data"),
                message_data: row.get("message_data"),
                primary_type: row.get("primary_type"),
                signature: row.get("signature"),
                chain_id: row.get("chain_id"),
                signed_at: row.get::<_, DateTime<Utc>>("signed_at"),
            });
        }

        let result_count = results.len();
        Ok(PagingResult::new(results, paging_context.next(result_count), paging_context.previous()))
    }
}
