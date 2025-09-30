use serde_json;
use tokio_postgres::Row;

use crate::{shared::common_types::EvmAddress, transaction::types::Transaction};

pub fn build_transaction_from_transaction_view(row: &Row) -> Transaction {
    let to: EvmAddress = row.get("to");
    let from: EvmAddress = row.get("from");

    Transaction {
        id: row.get("id"),
        relayer_id: row.get("relayer_id"),
        to,
        from,
        value: row.get("value"),
        chain_id: row.get("chain_id"),
        data: row.get("data"),
        nonce: row.get("nonce"),
        gas_limit: row.get("gas_limit"),
        status: row.get("status"),
        blobs: row.get("blobs"),
        known_transaction_hash: row.get("hash"),
        queued_at: row.get("queued_at"),
        expires_at: row.get("expires_at"),
        sent_at: row.get("sent_at"),
        confirmed_at: row.get("confirmed_at"),
        sent_with_gas: row
            .get::<_, Option<serde_json::Value>>("sent_with_gas")
            .and_then(|v| serde_json::from_value(v).ok()),
        sent_with_blob_gas: row
            .get::<_, Option<serde_json::Value>>("sent_with_blob_gas")
            .and_then(|v| serde_json::from_value(v).ok()),
        mined_at: row.get("mined_at"),
        mined_at_block_number: row.get("block_number"),
        speed: row.get("speed"),
        sent_with_max_priority_fee_per_gas: row.get("sent_max_priority_fee_per_gas"),
        sent_with_max_fee_per_gas: row.get("sent_max_fee_per_gas"),
        is_noop: to == from,
        external_id: row.get("external_id"),
        cancelled_by_transaction_id: row.get("cancelled_by_transaction_id"),
    }
}
