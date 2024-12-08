use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::{
    TransactionData, TransactionHash, TransactionId, TransactionNonce, TransactionSpeed,
    TransactionStatus, TransactionValue,
};
use crate::{
    gas::types::{GasLimit, GasPrice, MaxFee, MaxPriorityFee},
    relayer::types::Relayer,
    shared::{
        common_types::{ApiKey, BlockHash, BlockNumber, EvmAddress},
        serializers::{serialize_system_time, serialize_system_time_option},
    },
};

#[derive(Debug, Deserialize, Serialize)]
pub struct RelayerTransaction {
    /// The unique identifier for the transaction
    pub id: TransactionId,

    /// The relayer sent from
    pub relayer: Relayer,

    /// The API key used to submit the transaction
    #[serde(rename = "apiKey")]
    pub api_key: ApiKey,

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
    #[serde(rename = "maxPriorityFeePerGas", skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<MaxPriorityFee>,

    /// The maximum fee per gas of the transaction
    #[serde(rename = "maxFeePerGas", skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<MaxFee>,

    /// The block hash the transaction was included in
    #[serde(rename = "blockHash", skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<BlockHash>,

    /// The block number the transaction was included in
    #[serde(rename = "blockNumber", skip_serializing_if = "Option::is_none")]
    pub block_number: Option<BlockNumber>,

    /// The hash of the transaction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<TransactionHash>,

    /// The speed at which the transaction was relayed
    pub speed: TransactionSpeed,

    /// The status of the transaction
    pub status: TransactionStatus,

    /// The time the transaction will expire
    #[serde(rename = "expiresAt", serialize_with = "serialize_system_time")]
    pub expiries_at: SystemTime,

    /// The time the transaction was queued
    #[serde(rename = "queuedAt", serialize_with = "serialize_system_time")]
    pub queued_at: SystemTime,

    /// The time the transaction was mined
    #[serde(
        rename = "minedAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option"
    )]
    pub mined_at: Option<SystemTime>,

    /// The time the transaction was failed
    #[serde(
        rename = "failedAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option"
    )]
    pub failed_at: Option<SystemTime>,

    /// The time the transaction was sent
    #[serde(
        rename = "sentAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option"
    )]
    pub sent_at: Option<SystemTime>,

    /// The time the transaction was confirmed
    #[serde(
        rename = "confirmedAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option"
    )]
    pub confirmed_at: Option<SystemTime>,
}
