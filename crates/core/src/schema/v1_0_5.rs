use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.5.
/// Adds the per-transaction gas price ceiling columns honored by the gas bump loop.
pub async fn apply_v1_0_5_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        ALTER TABLE relayer.transaction
            ADD COLUMN IF NOT EXISTS gas_price_ceiling NUMERIC(80) NULL,
            ADD COLUMN IF NOT EXISTS gas_price_ceiling_behavior TEXT NULL,
            ADD COLUMN IF NOT EXISTS gas_price_ceiling_hit BOOLEAN NOT NULL DEFAULT FALSE;

        ALTER TABLE relayer.transaction_audit_log
            ADD COLUMN IF NOT EXISTS gas_price_ceiling NUMERIC(80) NULL,
            ADD COLUMN IF NOT EXISTS gas_price_ceiling_behavior TEXT NULL,
            ADD COLUMN IF NOT EXISTS gas_price_ceiling_hit BOOLEAN NOT NULL DEFAULT FALSE;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
