use crate::postgres::{PostgresClient, PostgresError};

/// Applies the RRelayer database schema version 1.0.3.
/// Adds persistent cron job run state so schedules survive process restarts.
pub async fn apply_v1_0_3_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    let schema_sql = r#"
        CREATE TABLE IF NOT EXISTS relayer.cron_job_state (
            project_name VARCHAR(255) NOT NULL,
            job_name VARCHAR(255) NOT NULL,
            last_ran_at TIMESTAMPTZ NOT NULL,
            updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
            PRIMARY KEY (project_name, job_name)
        );
    "#;

    client.batch_execute(schema_sql).await?;
    Ok(())
}
