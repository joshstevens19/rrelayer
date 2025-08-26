use std::{fmt::Display, time::SystemTime};

use alloy::{
    consensus::{
        TxEip1559, TxEip4844, TxEip4844Variant, TxEip4844WithSidecar, TxLegacy, TypedTransaction,
    },
    eips::eip2930::AccessList,
    primitives::TxKind,
};
use alloy_eips::eip4844::{
    builder::{SidecarBuilder, SimpleCoder},
    Blob,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactionConversionError {
    #[error("No gas price found in transaction")]
    NoGasPrice,
    #[error("No blob gas price found in transaction")]
    NoBlobGasPrice,
    #[error("No blobs found in transaction")]
    NoBlobs,
    #[error("Failed to build blob sidecar: {0}")]
    BlobSidecarBuild(String),
    #[error("Gas limit not set")]
    NoGasLimit,
}

use super::{
    TransactionData, TransactionHash, TransactionId, TransactionNonce, TransactionSpeed,
    TransactionStatus, TransactionValue,
};
use crate::{
    gas::{
        blob_gas_oracle::BlobGasPriceResult,
        fee_estimator::base::GasPriceResult,
        types::{GasLimit, MaxFee, MaxPriorityFee},
    },
    network::types::ChainId,
    relayer::types::RelayerId,
    shared::{
        common_types::{ApiKey, EvmAddress},
        serializers::{
            deserialize_system_time, deserialize_system_time_option, serialize_system_time,
            serialize_system_time_option,
        },
    },
};

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Transaction {
    pub id: TransactionId,

    #[serde(rename = "relayerId")]
    pub relayer_id: RelayerId,

    pub to: EvmAddress,

    pub from: EvmAddress,

    pub value: TransactionValue,

    pub data: TransactionData,

    pub nonce: TransactionNonce,

    pub chain_id: ChainId,

    pub gas_limit: Option<GasLimit>,

    pub status: TransactionStatus,

    #[serde(rename = "txHash", skip_serializing_if = "Option::is_none", default)]
    pub blobs: Option<Vec<Blob>>,

    #[serde(rename = "txHash", skip_serializing_if = "Option::is_none", default)]
    pub known_transaction_hash: Option<TransactionHash>,

    #[serde(
        rename = "queuedAt",
        serialize_with = "serialize_system_time",
        deserialize_with = "deserialize_system_time"
    )]
    pub queued_at: SystemTime,

    #[serde(
        rename = "expiresAt",
        serialize_with = "serialize_system_time",
        deserialize_with = "deserialize_system_time"
    )]
    pub expires_at: SystemTime,

    #[serde(
        rename = "sentAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option",
        deserialize_with = "deserialize_system_time_option",
        default
    )]
    pub sent_at: Option<SystemTime>,

    #[serde(
        rename = "confirmedAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option",
        deserialize_with = "deserialize_system_time_option",
        default
    )]
    pub confirmed_at: Option<SystemTime>,

    #[serde(rename = "sentWithGas", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_gas: Option<GasPriceResult>,

    #[serde(rename = "sentWithBlobGas", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_blob_gas: Option<BlobGasPriceResult>,

    #[serde(
        rename = "minedAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option",
        deserialize_with = "deserialize_system_time_option",
        default
    )]
    pub mined_at: Option<SystemTime>,

    pub speed: TransactionSpeed,

    #[serde(rename = "maxPriorityFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_priority_fee_per_gas: Option<MaxPriorityFee>,

    #[serde(rename = "maxFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_fee_per_gas: Option<MaxFee>,

    #[serde(skip_serializing, skip_deserializing, default)]
    pub is_noop: bool,

    #[serde(skip_serializing, skip_deserializing, default)]
    pub from_api_key: ApiKey,

    pub external_id: Option<String>,
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transaction {}", self.id)
    }
}

impl Transaction {
    /// Checks if this transaction has been previously sent to the network.
    ///
    /// # Returns
    /// * `bool` - True if the transaction has a sent_at timestamp
    pub fn has_been_sent_before(&self) -> bool {
        self.sent_at.is_some()
    }

    fn is_eip1559(&self) -> bool {
        self.sent_with_max_priority_fee_per_gas.is_some()
            && self.sent_with_max_fee_per_gas.is_some()
    }

    /// Converts this transaction to an EIP-1559 typed transaction.
    ///
    /// Creates an EIP-1559 transaction with max priority fee and max fee per gas.
    ///
    /// # Arguments
    /// * `override_gas_price` - Optional gas price to override stored values
    ///
    /// # Returns
    /// * `Ok(TypedTransaction)` - EIP-1559 typed transaction
    /// * `Err(TransactionConversionError)` - If gas price information is missing
    pub fn to_eip1559_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price,
            None => self.sent_with_gas.as_ref().ok_or(TransactionConversionError::NoGasPrice)?,
        };

        Ok(TypedTransaction::Eip1559(TxEip1559 {
            to: TxKind::Call(self.to.into()),
            value: self.value.clone().into(),
            input: self.data.clone().into(),
            // TODO: fix
            // gas_limit: self.gas_limit.unwrap().into(),
            gas_limit: 210000,
            nonce: self.nonce.into(),
            max_priority_fee_per_gas: gas_price_result.max_priority_fee.clone().into(),
            max_fee_per_gas: gas_price_result.max_fee.into(),
            chain_id: self.chain_id.into(),
            access_list: AccessList::default(),
        }))
    }

    pub fn to_legacy_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price.legacy_gas_price(),
            None => self
                .sent_with_gas
                .as_ref()
                .ok_or(TransactionConversionError::NoGasPrice)?
                .legacy_gas_price(),
        };

        let gas_limit = self.gas_limit.ok_or(TransactionConversionError::NoGasLimit)?;

        Ok(TypedTransaction::Legacy(TxLegacy {
            to: TxKind::Call(self.to.into()),
            value: self.value.clone().into(),
            input: self.data.clone().into(),
            gas_limit: gas_limit.into(),
            nonce: self.nonce.into(),
            gas_price: gas_price_result.into(),
            chain_id: Some(self.chain_id.into()),
        }))
    }

    pub fn to_blob_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
        override_blob_gas_price: Option<&BlobGasPriceResult>,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price,
            None => self.sent_with_gas.as_ref().ok_or(TransactionConversionError::NoGasPrice)?,
        };

        let blob_gas_price = match override_blob_gas_price {
            Some(blob_price) => blob_price.blob_gas_price,
            None => {
                self.sent_with_blob_gas
                    .as_ref()
                    .ok_or(TransactionConversionError::NoBlobGasPrice)?
                    .blob_gas_price
            }
        };

        let blobs = self.blobs.clone().ok_or(TransactionConversionError::NoBlobs)?;

        let builder: SidecarBuilder<SimpleCoder> =
            blobs.iter().map(|blob| blob.as_slice()).collect();
        let sidecar = builder
            .build()
            .map_err(|e| TransactionConversionError::BlobSidecarBuild(e.to_string()))?;

        let blob_versioned_hashes = sidecar.versioned_hashes().collect::<Vec<_>>();

        let tx = TxEip4844 {
            chain_id: self.chain_id.into(),
            nonce: self.nonce.into(),
            max_priority_fee_per_gas: gas_price_result.max_priority_fee.into(),
            max_fee_per_gas: gas_price_result.max_fee.into(),
            // TODO: fix
            // gas_limit: self.gas_limit.unwrap().into(),
            gas_limit: 210000,
            to: self.to.into(),
            value: self.value.clone().into(),
            access_list: Default::default(),
            blob_versioned_hashes,
            max_fee_per_blob_gas: blob_gas_price.into(),
            input: self.data.clone().into(),
        };

        Ok(TypedTransaction::Eip4844(TxEip4844Variant::TxEip4844WithSidecar(
            TxEip4844WithSidecar { tx, sidecar },
        )))
    }

    /// Checks if this is a blob transaction (EIP-4844).
    ///
    /// # Returns
    /// * `bool` - True if the transaction has blob data
    pub fn is_blob_transaction(&self) -> bool {
        self.blobs.is_some()
    }
}
