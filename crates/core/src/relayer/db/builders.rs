use tokio_postgres::Row;

use crate::relayer::types::Relayer;

pub fn build_relayer(row: &Row) -> Relayer {
    Relayer {
        id: row.get("id"),
        name: row.get("name"),
        chain_id: row.get("chain_id"),
        address: row.get("address"),
        wallet_index: row.get::<_, i32>("wallet_index") as u32,
        max_gas_price: row.get("max_gas_price_cap"),
        paused: row.get("paused"),
        eip_1559_enabled: row.get("eip_1559_enabled"),
        created_at: row.get("created_at"),
    }
}
