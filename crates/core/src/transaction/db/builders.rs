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
        // TODO! load blobs from db
        blobs: None,
        known_transaction_hash: row.get("hash"),
        queued_at: row.get("queued_at"),
        expires_at: row.get("expires_at"),
        sent_at: row.get("sent_at"),
        sent_with_gas: None,
        mined_at: row.get("mined_at"),
        speed: row.get("speed"),
        sent_with_max_priority_fee_per_gas: row.get("sent_max_priority_fee_per_gas"),
        sent_with_max_fee_per_gas: row.get("sent_max_fee_per_gas"),
        is_noop: to == from,
        from_api_key: row.get("api_key"),
    }
}
