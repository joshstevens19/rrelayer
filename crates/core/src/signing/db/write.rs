use serde::{Deserialize, Serialize};

use crate::common_types::Signature;
use crate::network::types::ChainId;
use crate::postgres::{PostgresClient, PostgresError};
use crate::relayer::types::RelayerId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordSignedTypedDataRequest {
    pub relayer_id: RelayerId,
    pub domain_data: serde_json::Value,
    pub message_data: serde_json::Value,
    pub primary_type: String,
    pub signature: Signature,
    pub chain_id: ChainId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordSignedTextRequest {
    pub relayer_id: RelayerId,
    pub message: String,
    pub signature: Signature,
    pub chain_id: ChainId,
}

impl PostgresClient {
    /// Records a signed text message in the database.
    ///
    /// # Arguments
    /// * `request` - The signed text record to save
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The ID of the created record
    /// * `Err(PostgresError)` - If the database operation fails
    pub async fn record_signed_text(
        &self,
        request: &RecordSignedTextRequest,
    ) -> Result<(), PostgresError> {
        let query = r#"
            INSERT INTO signing.text_history (relayer_id, message, signature, chain_id)
            VALUES ($1, $2, $3, $4);
        "#;

        let conn = self.pool.get().await?;
        conn.execute(
            query,
            &[&request.relayer_id, &request.message, &request.signature, &request.chain_id],
        )
        .await?;

        Ok(())
    }

    /// Records a signed typed data (EIP-712) message in the database.
    ///
    /// # Arguments
    /// * `request` - The signed typed data record to save
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The ID of the created record
    /// * `Err(PostgresError)` - If the database operation fails
    pub async fn record_signed_typed_data(
        &self,
        request: &RecordSignedTypedDataRequest,
    ) -> Result<(), PostgresError> {
        let query = r#"
            INSERT INTO signing.typed_data_history (relayer_id, domain_data, message_data, primary_type, signature, chain_id)
            VALUES ($1, $2, $3, $4, $5, $6);
        "#;

        let conn = self.pool.get().await?;
        conn.execute(
            query,
            &[
                &request.relayer_id,
                &request.domain_data,
                &request.message_data,
                &request.primary_type,
                &request.signature,
                &request.chain_id,
            ],
        )
        .await?;

        Ok(())
    }
}
