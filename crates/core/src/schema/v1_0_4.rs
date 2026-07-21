use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.4.
/// Enforces uniqueness of external_id per relayer so sends are idempotent.
pub async fn apply_v1_0_4_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_transaction_relayer_external_id
        ON relayer.transaction(relayer_id, external_id) WHERE external_id IS NOT NULL;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
