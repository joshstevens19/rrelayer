use crate::postgres::{PostgresClient, PostgresError};

const SCHEMA_SQL: &str = r#"
    CREATE INDEX IF NOT EXISTS idx_transaction_audit_attempt_lookup
    ON relayer.transaction_audit_log(id, history_id)
    WHERE hash IS NOT NULL AND sent_at IS NOT NULL;

    DROP INDEX IF EXISTS relayer.idx_relayer_live_normal_wallet_namespace;

    CREATE UNIQUE INDEX IF NOT EXISTS idx_relayer_live_normal_root_wallet_namespace
    ON relayer.record(chain_id, wallet_index)
    WHERE deleted = FALSE
      AND is_private_key = FALSE
      AND wallet_index >= 0
      AND cloned_from_chain_id IS NULL;
"#;

/// Adds transaction integrity indexes for ordered attempt recovery and stable
/// live normal-wallet namespaces. Both indexes are idempotent. Cloned rows are
/// aliases for an existing signing identity, so the root-identity uniqueness
/// constraint deliberately excludes them.
pub async fn apply_v1_0_4_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    client.batch_execute(SCHEMA_SQL).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SCHEMA_SQL;

    #[test]
    fn live_root_wallet_indexes_are_unique_without_rejecting_canonical_clones() {
        assert!(SCHEMA_SQL.contains("CREATE UNIQUE INDEX"));
        assert!(SCHEMA_SQL.contains("wallet_index >= 0"));
        assert!(SCHEMA_SQL.contains("cloned_from_chain_id IS NULL"));
    }
}
