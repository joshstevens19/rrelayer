use crate::{
    postgres::{PostgresClient, PostgresError},
    schema::v1_0_0::apply_v1_0_0_schema,
};

mod v1_0_0;

/// Applies the database schema to the database.
pub async fn apply_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    apply_v1_0_0_schema(client).await
}
