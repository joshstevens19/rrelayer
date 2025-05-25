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
        serializers::{serialize_system_time, serialize_system_time_option},
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

    pub blobs: Option<Vec<Blob>>,

    #[serde(rename = "txHash", skip_serializing_if = "Option::is_none", default)]
    pub known_transaction_hash: Option<TransactionHash>,

    #[serde(rename = "queuedAt", serialize_with = "serialize_system_time")]
    pub queued_at: SystemTime,

    #[serde(rename = "expiresAt", serialize_with = "serialize_system_time")]
    pub expires_at: SystemTime,

    #[serde(
        rename = "sentAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option",
        default
    )]
    pub sent_at: Option<SystemTime>,

    #[serde(rename = "sentWithGas", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_gas: Option<GasPriceResult>,

    #[serde(rename = "sentWithBlobGas", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_blob_gas: Option<BlobGasPriceResult>,

    #[serde(
        rename = "minedAt",
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_system_time_option",
        default
    )]
    pub mined_at: Option<SystemTime>,

    pub speed: TransactionSpeed,

    #[serde(rename = "maxPriorityFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_priority_fee_per_gas: Option<MaxPriorityFee>,

    #[serde(rename = "maxFee", skip_serializing_if = "Option::is_none", default)]
    pub sent_with_max_fee_per_gas: Option<MaxFee>,

    #[serde(skip_serializing)]
    pub is_noop: bool,

    #[serde(skip_serializing)]
    pub from_api_key: ApiKey,
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Transaction {}", self.id)
    }
}

impl Transaction {
    pub fn has_been_sent_before(&self) -> bool {
        self.sent_at.is_some()
    }

    fn is_eip1559(&self) -> bool {
        self.sent_with_max_priority_fee_per_gas.is_some() &&
            self.sent_with_max_fee_per_gas.is_some()
    }

    pub fn to_eip1559_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
    ) -> TypedTransaction {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price,
            None => self.sent_with_gas.as_ref().unwrap(),
        };

        TypedTransaction::Eip1559(TxEip1559 {
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
        })
    }

    pub fn to_legacy_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
    ) -> TypedTransaction {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price.legacy_gas_price(),
            None => self.sent_with_gas.as_ref().unwrap().legacy_gas_price(),
        };

        TypedTransaction::Legacy(TxLegacy {
            to: TxKind::Call(self.to.into()),
            value: self.value.clone().into(),
            input: self.data.clone().into(),
            gas_limit: self.gas_limit.unwrap().into(),
            nonce: self.nonce.into(),
            gas_price: gas_price_result.into(),
            chain_id: Some(self.chain_id.into()),
        })
    }

    pub fn to_blob_typed_transaction(
        &self,
        override_gas_price: Option<&GasPriceResult>,
        override_blob_gas_price: Option<&BlobGasPriceResult>,
    ) -> TypedTransaction {
        let gas_price_result = match override_gas_price {
            Some(gas_price) => gas_price,
            None => self.sent_with_gas.as_ref().expect("No gas price found"),
        };

        let blob_gas_price = match override_blob_gas_price {
            Some(blob_price) => blob_price.blob_gas_price,
            None => {
                self.sent_with_blob_gas.as_ref().expect("No blob gas price found").blob_gas_price
            }
        };

        let blobs = self.blobs.clone().expect("No blobs found - should not be possible");

        let builder: SidecarBuilder<SimpleCoder> =
            blobs.iter().map(|blob| blob.as_slice()).collect();
        let sidecar = builder.build().expect("Failed to build blobs");

        let blob_versioned_hashes = sidecar.versioned_hashes().collect::<Vec<_>>();

        let tx = TxEip4844 {
            chain_id: self.chain_id.into(),
            nonce: self.nonce.into(),
            max_priority_fee_per_gas: gas_price_result.max_priority_fee.into(),
            max_fee_per_gas: gas_price_result.max_fee.into(),
            gas_limit: self.gas_limit.unwrap().into(),
            to: self.to.into(),
            value: self.value.clone().into(),
            access_list: Default::default(),
            blob_versioned_hashes,
            max_fee_per_blob_gas: blob_gas_price.into(),
            input: self.data.clone().into(),
        };

        TypedTransaction::Eip4844(TxEip4844Variant::TxEip4844WithSidecar(TxEip4844WithSidecar {
            tx,
            sidecar,
        }))
    }

    pub fn is_blob_transaction(&self) -> bool {
        self.blobs.is_some()
    }
}
