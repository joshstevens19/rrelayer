use tokio_postgres::Row;

use crate::relayer::types::Relayer;

/// Builds a Relayer struct from a PostgreSQL database row.
///
/// This function extracts relayer data from a database query result row and constructs
/// a Relayer instance with proper type conversions and field mappings.
///
/// # Arguments
/// * `row` - A PostgreSQL row containing relayer data from the database
///
/// # Returns
/// * A fully constructed Relayer instance with data from the database row
pub fn build_relayer(row: &Row) -> Relayer {
    Relayer {
        id: row.get("id"),
        name: row.get("name"),
        chain_id: row.get("chain_id"),
        address: row.get("address"),
        wallet_index: row.get::<_, i32>("wallet_index") as u32,
        max_gas_price: row.get("max_gas_price_cap"),
        paused: row.get("paused"),
        allowlisted_only: row.get("allowlisted_addresses_only"),
        eip_1559_enabled: row.get("eip_1559_enabled"),
        created_at: row.get("created_at"),
    }
}
