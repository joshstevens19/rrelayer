use crate::{
    postgres::{PostgresClient, PostgresError},
    schema::v1_0_0::apply_v1_0_0_schema,
};

mod v1_0_0;

/// Applies the database schema to the PostgreSQL database.
///
/// Creates all necessary tables, indexes, constraints, and types required
/// for the RRelayer application. Currently applies schema version 1.0.0.
///
/// The schema includes:
/// - Authentication tables for users and roles
/// - Network configuration tables
/// - Relayer management tables
/// - Transaction tracking and audit tables
/// - Foreign key constraints and indexes
///
/// # Arguments
/// * `client` - PostgreSQL client with appropriate permissions
///
/// # Returns
/// * `Ok(())` - If schema is applied successfully
/// * `Err(PostgresError)` - If schema application fails
///
/// # Note
/// This function is idempotent and can be run multiple times safely.
/// Existing tables and data will not be affected.
pub async fn apply_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    apply_v1_0_0_schema(client).await
}
