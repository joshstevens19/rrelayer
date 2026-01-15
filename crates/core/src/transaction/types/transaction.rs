use std::fmt::Display;

use alloy::{
    consensus::{
        BlobTransactionSidecarVariant, TxEip1559, TxEip4844, TxEip4844Variant,
        TxEip4844WithSidecar, TxLegacy, TypedTransaction,
    },
    eips::eip2930::AccessList,
    primitives::TxKind,
};
use alloy_eips::eip4844::{
    builder::{SidecarBuilder, SimpleCoder},
    BlobTransactionSidecar,
};
use chrono::{DateTime, Utc};
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
    TransactionBlob, TransactionData, TransactionHash, TransactionId, TransactionNonce,
    TransactionSpeed, TransactionStatus, TransactionValue,
};
use crate::common_types::BlockNumber;
use crate::{
    gas::{BlobGasPriceResult, GasLimit, GasPriceResult, MaxFee, MaxPriorityFee},
    network::ChainId,
    relayer::RelayerId,
    shared::common_types::EvmAddress,
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

    #[serde(rename = "chainId")]
    pub chain_id: ChainId,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub gas_limit: Option<GasLimit>,

    pub status: TransactionStatus,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blobs: Option<Vec<TransactionBlob>>,

    #[serde(rename = "txHash", skip_serializing_if = "Option::is_none", default)]
    pub known_transaction_hash: Option<TransactionHash>,

    #[serde(rename = "queuedAt")]
    pub queued_at: DateTime<Utc>,

    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,

    #[serde(rename = "sentAt", skip_serializing_if = "Option::is_none", default)]
    pub sent_at: Option<DateTime<Utc>>,

    #[serde(rename = "confirmedAt", skip_serializing_if = "Option::is_none", default)]
    pub confirmed_at: Option<DateTime<Utc>>,

    #[serde(rename = "sentWithGas", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_gas: Option<GasPriceResult>,

    #[serde(rename = "sentWithBlobGas", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_blob_gas: Option<BlobGasPriceResult>,

    #[serde(rename = "minedAt", skip_serializing_if = "Option::is_none", default)]
    pub mined_at: Option<DateTime<Utc>>,

    #[serde(rename = "minedAtBlockNumber", skip_serializing_if = "Option::is_none", default)]
    pub mined_at_block_number: Option<BlockNumber>,

    pub speed: TransactionSpeed,

    #[serde(rename = "maxPriorityFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_priority_fee_per_gas: Option<MaxPriorityFee>,

    #[serde(rename = "maxFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_fee_per_gas: Option<MaxFee>,

    #[serde(rename = "isNoop")]
    pub is_noop: bool,

    #[serde(rename = "externalId", skip_serializing_if = "Option::is_none", default)]
    pub external_id: Option<String>,

    #[serde(rename = "cancelledByTransactionId", skip_serializing_if = "Option::is_none", default)]
    pub cancelled_by_transaction_id: Option<TransactionId>,
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

    /// Converts this transaction to an EIP-1559 typed transaction.
    ///
    /// Creates an EIP-1559 transaction with max priority fee and max fee per gas.
    ///
    /// # Arguments
    /// * `override_gas_price` - Optional gas price to override stored values
    /// * `override_gas_limit` - Optional gas limit to override stored values
    ///
    /// # Returns
    /// * `Ok(TypedTransaction)` - EIP-1559 typed transaction
    /// * `Err(TransactionConversionError)` - If gas price information is missing
    pub fn to_eip1559_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        self.to_eip1559_typed_transaction_with_gas_limit(override_gas_price, None)
    }

    /// Converts this transaction to an EIP-1559 typed transaction with optional gas limit override.
    ///
    /// Creates an EIP-1559 transaction with max priority fee and max fee per gas.
    ///
    /// # Arguments
    /// * `override_gas_price` - Optional gas price to override stored values
    /// * `override_gas_limit` - Optional gas limit to override stored values
    ///
    /// # Returns
    /// * `Ok(TypedTransaction)` - EIP-1559 typed transaction
    /// * `Err(TransactionConversionError)` - If gas price or gas limit information is missing
    pub fn to_eip1559_typed_transaction_with_gas_limit(
        &self,
        override_gas_price: Option<&GasPriceResult>,
        override_gas_limit: Option<GasLimit>,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price,
            None => self.sent_with_gas.as_ref().ok_or(TransactionConversionError::NoGasPrice)?,
        };

        let gas_limit = match override_gas_limit {
            Some(limit) => limit,
            None => self.gas_limit.ok_or(TransactionConversionError::NoGasLimit)?,
        };

        Ok(TypedTransaction::Eip1559(TxEip1559 {
            to: TxKind::Call(self.to.into()),
            value: self.value.into(),
            input: self.data.clone().into(),
            gas_limit: gas_limit.into(),
            nonce: self.nonce.into(),
            max_priority_fee_per_gas: gas_price_result.max_priority_fee.into(),
            max_fee_per_gas: gas_price_result.max_fee.into(),
            chain_id: self.chain_id.into(),
            access_list: AccessList::default(),
        }))
    }

    pub fn to_legacy_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        self.to_legacy_typed_transaction_with_gas_limit(override_gas_price, None)
    }

    pub fn to_legacy_typed_transaction_with_gas_limit(
        &self,
        override_gas_price: Option<&GasPriceResult>,
        override_gas_limit: Option<GasLimit>,
    ) -> Result<TypedTransaction, TransactionConversionError> {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price.legacy_gas_price(),
            None => self
                .sent_with_gas
                .as_ref()
                .ok_or(TransactionConversionError::NoGasPrice)?
                .legacy_gas_price(),
        };

        let gas_limit = match override_gas_limit {
            Some(limit) => limit,
            None => self.gas_limit.ok_or(TransactionConversionError::NoGasLimit)?,
        };

        Ok(TypedTransaction::Legacy(TxLegacy {
            to: TxKind::Call(self.to.into()),
            value: self.value.into(),
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
        self.to_blob_typed_transaction_with_gas_limit(
            override_gas_price,
            override_blob_gas_price,
            None,
        )
    }

    pub fn to_blob_typed_transaction_with_gas_limit(
        &self,
        override_gas_price: Option<&GasPriceResult>,
        override_blob_gas_price: Option<&BlobGasPriceResult>,
        override_gas_limit: Option<GasLimit>,
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
        let sidecar: BlobTransactionSidecar = builder
            .build()
            .map_err(|e| TransactionConversionError::BlobSidecarBuild(e.to_string()))?;

        let gas_limit = match override_gas_limit {
            Some(limit) => limit,
            None => self.gas_limit.ok_or(TransactionConversionError::NoGasLimit)?,
        };
        let blob_versioned_hashes = sidecar.versioned_hashes().collect::<Vec<_>>();

        let tx = TxEip4844 {
            chain_id: self.chain_id.into(),
            nonce: self.nonce.into(),
            max_priority_fee_per_gas: gas_price_result.max_priority_fee.into(),
            max_fee_per_gas: gas_price_result.max_fee.into(),
            gas_limit: gas_limit.into(),
            to: self.to.into(),
            value: self.value.into(),
            access_list: Default::default(),
            blob_versioned_hashes,
            max_fee_per_blob_gas: blob_gas_price,
            input: self.data.clone().into(),
        };

        Ok(TypedTransaction::Eip4844(TxEip4844Variant::TxEip4844WithSidecar(
            TxEip4844WithSidecar { tx, sidecar: BlobTransactionSidecarVariant::Eip4844(sidecar) },
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
