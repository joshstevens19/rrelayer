use alloy_eips::eip4844::Blob;

use crate::{
    shared::common_types::{ApiKey, EvmAddress},
    transaction::types::{TransactionData, TransactionId, TransactionSpeed, TransactionValue},
};

#[derive(Clone, Debug)]
pub struct TransactionToSend {
    pub id: TransactionId,
    pub speed: TransactionSpeed,
    pub from_api_key: ApiKey,
    pub to: EvmAddress,
    pub value: TransactionValue,
    pub data: TransactionData,
    pub blobs: Option<Vec<Blob>>,
}

impl TransactionToSend {
    pub fn new(
        to: EvmAddress,
        api_key: String,
        value: TransactionValue,
        data: TransactionData,
        speed: Option<TransactionSpeed>,
        blobs: Option<Vec<Blob>>,
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
        }
    }
}
