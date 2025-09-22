use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    TransactionData, TransactionHash, TransactionId, TransactionNonce, TransactionSpeed,
    TransactionStatus, TransactionValue,
};
use crate::{
    gas::{GasLimit, GasPrice, MaxFee, MaxPriorityFee},
    relayer::Relayer,
    shared::common_types::{BlockHash, BlockNumber, EvmAddress},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct RelayerTransaction {
    /// The unique identifier for the transaction
    pub id: TransactionId,

    /// The relayer sent from
    pub relayer: Relayer,

    /// The address the transaction is being sent to
    pub to: EvmAddress,

    /// The nonce of the transaction
    pub nonce: TransactionNonce,

    /// The data of the transaction
    pub data: Option<TransactionData>,

    /// The value of the transaction
    pub value: Option<TransactionValue>,

    /// The gas limit of the transaction
    pub gas: Option<GasLimit>,

    /// The gas price of the transaction
    #[serde(rename = "gasPrice")]
    pub gas_price: Option<GasPrice>,

    /// The maximum priority fee per gas of the transaction
    #[serde(rename = "maxPriorityFeePerGas", skip_serializing_if = "Option::is_none", default)]
    pub max_priority_fee_per_gas: Option<MaxPriorityFee>,

    /// The maximum fee per gas of the transaction
    #[serde(rename = "maxFeePerGas", skip_serializing_if = "Option::is_none", default)]
    pub max_fee_per_gas: Option<MaxFee>,

    /// The block hash the transaction was included in
    #[serde(rename = "blockHash", skip_serializing_if = "Option::is_none", default)]
    pub block_hash: Option<BlockHash>,

    /// The block number the transaction was included in
    #[serde(rename = "blockNumber", skip_serializing_if = "Option::is_none", default)]
    pub block_number: Option<BlockNumber>,

    /// The hash of the transaction
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hash: Option<TransactionHash>,

    /// The speed at which the transaction was relayed
    pub speed: TransactionSpeed,

    /// The status of the transaction
    pub status: TransactionStatus,

    /// The time the transaction will expire
    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,

    /// The time the transaction was queued
    #[serde(rename = "queuedAt")]
    pub queued_at: DateTime<Utc>,

    /// The time the transaction was mined
    #[serde(rename = "minedAt", skip_serializing_if = "Option::is_none", default)]
    pub mined_at: Option<DateTime<Utc>>,

    /// The time the transaction was failed
    #[serde(rename = "failedAt", skip_serializing_if = "Option::is_none", default)]
    pub failed_at: Option<DateTime<Utc>>,

    /// The time the transaction was sent
    #[serde(rename = "sentAt", skip_serializing_if = "Option::is_none", default)]
    pub sent_at: Option<DateTime<Utc>>,

    /// The time the transaction was confirmed
    #[serde(rename = "confirmedAt", skip_serializing_if = "Option::is_none", default)]
    pub confirmed_at: Option<DateTime<Utc>>,
}
