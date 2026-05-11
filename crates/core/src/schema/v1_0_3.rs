use crate::postgres::{PostgresClient, PostgresError};

pub const TRANSACTION_EXTERNAL_ID_UNIQUE_INDEX: &str = "idx_transaction_relayer_external_id_unique";

/// Applies the RRelayer database schema version 1.0.3.
/// Adds per-relayer idempotency for non-null transaction external IDs.
pub async fn apply_v1_0_3_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = format!(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS {TRANSACTION_EXTERNAL_ID_UNIQUE_INDEX}
        ON relayer.transaction(relayer_id, external_id)
        WHERE external_id IS NOT NULL;
    "#
    );

    client.batch_execute(&schema_sql).await?;
    Ok(())
}
