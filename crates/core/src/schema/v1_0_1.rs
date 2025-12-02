use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.1.
pub async fn apply_v1_0_1_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        ALTER TABLE relayer.record
        ADD COLUMN IF NOT EXISTS cloned_from_chain_id BIGINT DEFAULT NULL;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
