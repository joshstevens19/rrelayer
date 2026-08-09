use crate::transaction::types::{GasPriceCeiling, TransactionBlob};
use crate::{
    shared::common_types::EvmAddress,
    transaction::types::{TransactionData, TransactionId, TransactionSpeed, TransactionValue},
};

#[derive(Clone, Debug)]
pub struct TransactionToSend {
    pub id: TransactionId,
    pub speed: TransactionSpeed,
    pub to: EvmAddress,
    pub value: TransactionValue,
    pub data: TransactionData,
    pub blobs: Option<Vec<TransactionBlob>>,
    pub external_id: Option<String>,
    pub gas_price_ceiling: Option<GasPriceCeiling>,
}

impl TransactionToSend {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        to: EvmAddress,
        value: TransactionValue,
        data: TransactionData,
        speed: Option<TransactionSpeed>,
        blobs: Option<Vec<TransactionBlob>>,
        external_id: Option<String>,
        gas_price_ceiling: Option<GasPriceCeiling>,
    ) -> Self {
        Self {
            id: TransactionId::new(),
            speed: speed.unwrap_or(TransactionSpeed::FAST),
            to,
            value,
            data,
            blobs,
            external_id,
            gas_price_ceiling,
        }
    }
}
