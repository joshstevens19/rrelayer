use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.2.
/// Adds indexes for transaction hash and external_id lookups.
pub async fn apply_v1_0_2_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_transaction_hash
        ON relayer.transaction(hash) WHERE hash IS NOT NULL;

        CREATE INDEX IF NOT EXISTS idx_transaction_external_id
        ON relayer.transaction(external_id) WHERE external_id IS NOT NULL;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}