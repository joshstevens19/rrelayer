use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.4.
/// Enforces uniqueness of external_id per relayer so sends are idempotent.
pub async fn apply_v1_0_4_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        -- Releases before this index allowed duplicate (relayer_id, external_id)
        -- rows. Keep the newest row (lookups are ORDER BY queued_at DESC) and
        -- clear the rest, or CREATE UNIQUE INDEX fails and every existing
        -- deployment with a duplicate refuses to boot.
        WITH ranked AS (
            SELECT id,
                   ROW_NUMBER() OVER (
                       PARTITION BY relayer_id, external_id
                       ORDER BY queued_at DESC, id DESC
                   ) AS rn
            FROM relayer.transaction
            WHERE external_id IS NOT NULL
        )
        UPDATE relayer.transaction AS t
        SET external_id = NULL
        FROM ranked
        WHERE t.id = ranked.id
          AND ranked.rn > 1;

        CREATE UNIQUE INDEX IF NOT EXISTS idx_transaction_relayer_external_id
        ON relayer.transaction(relayer_id, external_id) WHERE external_id IS NOT NULL;
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
