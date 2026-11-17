use crate::schema::v1_0_1::apply_v1_0_1_schema;
use crate::schema::v1_0_2::apply_v1_0_2_schema;
use crate::schema::v1_0_3::apply_v1_0_3_schema;
use crate::schema::v1_0_4::apply_v1_0_4_schema;
use crate::schema::v1_0_5::apply_v1_0_5_schema;
use crate::{
    postgres::{PostgresClient, PostgresError},
    schema::v1_0_0::apply_v1_0_0_schema,
};

mod v1_0_0;
mod v1_0_1;
mod v1_0_2;
mod v1_0_3;
mod v1_0_4;
mod v1_0_5;

/// Applies the database schema to the database.
pub async fn apply_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    apply_v1_0_0_schema(client).await?;
    apply_v1_0_1_schema(client).await?;
    apply_v1_0_2_schema(client).await?;
    apply_v1_0_3_schema(client).await?;
    apply_v1_0_4_schema(client).await?;
    apply_v1_0_5_schema(client).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::*;

    /// Exercises v1_0_4 against a real Postgres: pre-existing duplicate external ids
    /// are cleared down to the newest row, the unique index is created, and applying
    /// the schema again is a no-op. Hermetic `cargo test` skips this when
    /// `DATABASE_URL` is unset; the review run sets it and executes the body.
    #[tokio::test]
    async fn v1_0_4_dedupes_existing_external_ids_before_the_unique_index() {
        if std::env::var("DATABASE_URL").is_err() {
            eprintln!("DATABASE_URL unset; skipping v1_0_4 dedupe integration test");
            return;
        }

        let client = PostgresClient::new().await.expect("connect to postgres");
        apply_schema(&client).await.expect("schema applies on an empty or migrated database");

        let relayer_id = Uuid::new_v4();
        let older_id = Uuid::new_v4();
        let newer_id = Uuid::new_v4();

        client
            .execute(
                "DELETE FROM relayer.transaction WHERE relayer_id IN (SELECT id FROM relayer.record WHERE name = 'dedupe-ext-relayer')",
                &[],
            )
            .await
            .expect("cleanup transactions");
        client
            .execute("DELETE FROM relayer.record WHERE name = 'dedupe-ext-relayer'", &[])
            .await
            .expect("cleanup relayer");

        client
            .execute(
                "INSERT INTO relayer.record (id, name, chain_id, wallet_index) VALUES ($1, 'dedupe-ext-relayer', 1, 0)",
                &[&relayer_id],
            )
            .await
            .expect("insert relayer");

        client
            .batch_execute("DROP INDEX IF EXISTS relayer.idx_transaction_relayer_external_id")
            .await
            .expect("drop unique index so duplicates can be inserted");

        let older_at = Utc::now() - Duration::hours(2);
        let newer_at = Utc::now() - Duration::hours(1);
        let insert_tx = "
            INSERT INTO relayer.transaction (
                id, relayer_id, \"to\", \"from\", nonce, value, chain_id, speed, status,
                expires_at, queued_at, external_id
            ) VALUES (
                $1, $2, decode('1111111111111111111111111111111111111111', 'hex'),
                decode('2222222222222222222222222222222222222222', 'hex'), $3, 0, 1, 'FAST', 'PENDING',
                NOW() + INTERVAL '1 hour', $4, 'dedupe-ext'
            )
        ";
        client
            .execute(insert_tx, &[&older_id, &relayer_id, &1i64, &older_at])
            .await
            .expect("insert older duplicate");
        client
            .execute(insert_tx, &[&newer_id, &relayer_id, &2i64, &newer_at])
            .await
            .expect("insert newer duplicate");

        apply_v1_0_4_schema(&client)
            .await
            .expect("dedupe migration applies over existing duplicates");

        let rows = client
            .query(
                "SELECT id, external_id FROM relayer.transaction WHERE relayer_id = $1 ORDER BY queued_at ASC",
                &[&relayer_id],
            )
            .await
            .expect("read deduped rows");
        assert_eq!(rows.len(), 2);
        let older_external_id: Option<String> = rows[0].get("external_id");
        let newer_external_id: Option<String> = rows[1].get("external_id");
        assert!(older_external_id.is_none(), "older duplicate must lose its external id");
        assert_eq!(newer_external_id.as_deref(), Some("dedupe-ext"));
        assert_eq!(rows[1].get::<_, Uuid>("id"), newer_id);

        let index = client
            .query_one(
                "SELECT 1 FROM pg_indexes WHERE indexname = 'idx_transaction_relayer_external_id'",
                &[],
            )
            .await
            .expect("unique index exists");
        assert_eq!(index.get::<_, i32>(0), 1);

        let third_id = Uuid::new_v4();
        let duplicate_insert =
            client.execute(insert_tx, &[&third_id, &relayer_id, &3i64, &Utc::now()]).await;
        assert!(
            duplicate_insert.is_err(),
            "the unique index must reject a new duplicate external id"
        );

        apply_schema(&client).await.expect("re-applying the schema is idempotent");

        client
            .execute("DELETE FROM relayer.transaction WHERE relayer_id = $1", &[&relayer_id])
            .await
            .expect("cleanup transactions");
        client
            .execute("DELETE FROM relayer.record WHERE id = $1", &[&relayer_id])
            .await
            .expect("cleanup relayer");
    }
}
