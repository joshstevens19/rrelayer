use alloy_eips::eip4844::Blob;

use crate::{
    shared::common_types::{ApiKey, EvmAddress},
    transaction::types::{TransactionData, TransactionId, TransactionSpeed, TransactionValue},
};

/// Represents a transaction request to be sent through a relayer.
///
/// Contains all the necessary information for creating and sending a blockchain transaction.
#[derive(Clone, Debug)]
pub struct TransactionToSend {
    pub id: TransactionId,
    pub speed: TransactionSpeed,
    pub from_api_key: ApiKey,
    pub to: EvmAddress,
    pub value: TransactionValue,
    pub data: TransactionData,
    pub blobs: Option<Vec<Blob>>,
    pub external_id: Option<String>,
}

impl TransactionToSend {
    /// Creates a new transaction request.
    ///
    /// # Arguments
    /// * `to` - The recipient address
    /// * `api_key` - The API key of the sender
    /// * `value` - The ETH value to transfer
    /// * `data` - The transaction data/calldata
    /// * `speed` - Optional transaction speed tier (defaults to Medium)
    /// * `blobs` - Optional blob data for EIP-4844 transactions
    /// * `external_id` - Optional external reference ID
    ///
    /// # Returns
    /// * `TransactionToSend` - The constructed transaction request
    pub fn new(
        to: EvmAddress,
        api_key: String,
        value: TransactionValue,
        data: TransactionData,
        speed: Option<TransactionSpeed>,
        blobs: Option<Vec<Blob>>,
        external_id: Option<String>,
    ) -> Self {
        Self {
            id: TransactionId::new(),
            speed: speed.unwrap_or(TransactionSpeed::Fast),
            from_api_key: api_key,
            to,
            // from,
            value,
            data,
            blobs,
            external_id,
        }
    }
}
