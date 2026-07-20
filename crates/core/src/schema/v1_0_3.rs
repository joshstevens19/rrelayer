use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.3.
/// Adds a column for `authorization_list`.
pub async fn apply_v1_0_3_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        ALTER TABLE relayer.transaction
        ADD COLUMN IF NOT EXISTS authorization_list JSONB DEFAULT null;

        ALTER TABLE relayer.transaction_audit_log
        ADD COLUMN IF NOT EXISTS authorization_list JSONB DEFAULT null;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
