use crate::schema::v1_0_1::apply_v1_0_1_schema;
use crate::schema::v1_0_2::apply_v1_0_2_schema;
use crate::{
    postgres::{PostgresClient, PostgresError},
    schema::v1_0_0::apply_v1_0_0_schema,
};

mod v1_0_0;
mod v1_0_1;
mod v1_0_2;

/// Applies the database schema to the database.
pub async fn apply_schema(client: &PostgresClient) -> Result<(), PostgresError> {
    apply_v1_0_0_schema(client).await?;
    apply_v1_0_1_schema(client).await?;
    apply_v1_0_2_schema(client).await?;

    Ok(())
}
