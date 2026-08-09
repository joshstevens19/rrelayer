use std::fmt::Display;

use alloy::{
    consensus::{
        TxEip1559, TxEip4844, TxEip4844Variant, TxEip4844WithSidecar, TxLegacy, TypedTransaction,
    },
    eips::eip2930::AccessList,
    primitives::TxKind,
};
use alloy_eips::eip4844::{
    builder::{SidecarBuilder, SimpleCoder},
    BlobTransactionSidecar,
};
use alloy_eips::eip7594::BlobTransactionSidecarVariant;
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
    GasPriceCeiling, GasPriceCeilingOutcome, TransactionBlob, TransactionData, TransactionHash,
    TransactionId, TransactionNonce, TransactionSpeed, TransactionStatus, TransactionValue,
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

    /// Set when the node permanently rejected this transaction's payload and the queue
    /// replaced it with a same-nonce no-op; once that no-op mines the transaction
    /// resolves to FAILED (instead of MINED/EXPIRED) carrying this reason.
    #[serde(rename = "failedReason", skip_serializing_if = "Option::is_none", default)]
    pub failed_reason: Option<String>,

    /// Optional absolute per-transaction gas price ceiling honored on the initial send
    /// and through the gas bump loop. Cleared when the transaction is converted to a
    /// close-out no-op - the reserved nonce must always be consumable at market price.
    #[serde(rename = "gasPriceCeiling", skip_serializing_if = "Option::is_none", default)]
    pub gas_price_ceiling: Option<GasPriceCeiling>,

    /// True once the gas price ceiling actually bound a bid (a bump was frozen, a bid
    /// was clamped, or the first bid was refused). Lets callers distinguish "expired
    /// because the ceiling held the price down" from a plain expiry.
    #[serde(rename = "gasPriceCeilingHit", default)]
    pub gas_price_ceiling_hit: bool,
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

    /// Applies this transaction's gas price ceiling (if any) to a freshly computed bid,
    /// clamping it in place for cap behavior and recording on the transaction when the
    /// ceiling bound it (`gas_price_ceiling_hit`).
    pub fn apply_gas_price_ceiling(
        &mut self,
        gas_price: &mut GasPriceResult,
    ) -> GasPriceCeilingOutcome {
        let Some(ceiling) = self.gas_price_ceiling else {
            return GasPriceCeilingOutcome::WithinCeiling;
        };

        let outcome = ceiling.apply(gas_price, self.sent_with_gas.as_ref());
        if outcome != GasPriceCeilingOutcome::WithinCeiling {
            self.gas_price_ceiling_hit = true;
        }

        outcome
    }
}

#[cfg(test)]
pub(crate) mod test_fixtures {
    use chrono::Utc;

    use super::*;
    use crate::gas::{MaxFee, MaxPriorityFee};

    /// Bare transaction for queue/ceiling unit tests - no database or network involved.
    pub(crate) fn test_transaction(nonce: u64, status: TransactionStatus) -> Transaction {
        Transaction {
            id: TransactionId::new(),
            relayer_id: RelayerId::new(),
            to: EvmAddress::from(alloy::primitives::Address::ZERO),
            from: EvmAddress::from(alloy::primitives::Address::repeat_byte(0x11)),
            value: TransactionValue::zero(),
            data: TransactionData::empty(),
            nonce: TransactionNonce::new(nonce),
            chain_id: ChainId::new(31337),
            gas_limit: None,
            status,
            blobs: None,
            known_transaction_hash: None,
            queued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            sent_at: None,
            confirmed_at: None,
            sent_with_gas: None,
            sent_with_blob_gas: None,
            mined_at: None,
            mined_at_block_number: None,
            speed: TransactionSpeed::FAST,
            sent_with_max_priority_fee_per_gas: None,
            sent_with_max_fee_per_gas: None,
            is_noop: false,
            external_id: None,
            cancelled_by_transaction_id: None,
            failed_reason: None,
            gas_price_ceiling: None,
            gas_price_ceiling_hit: false,
        }
    }

    pub(crate) fn test_gas_price(max_fee: u128, max_priority_fee: u128) -> GasPriceResult {
        GasPriceResult {
            max_fee: MaxFee::new(max_fee),
            max_priority_fee: MaxPriorityFee::new(max_priority_fee),
            min_wait_time_estimate: None,
            max_wait_time_estimate: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_fixtures::{test_gas_price, test_transaction};
    use super::*;
    use crate::gas::GasPrice;
    use crate::transaction::types::GasPriceCeilingBehavior;

    fn ceiling(max_price: u128, behavior: GasPriceCeilingBehavior) -> GasPriceCeiling {
        GasPriceCeiling { max_price: GasPrice::new(max_price), behavior }
    }

    #[test]
    fn apply_gas_price_ceiling_without_a_ceiling_is_a_noop() {
        let mut transaction = test_transaction(0, TransactionStatus::PENDING);
        let mut bid = test_gas_price(120, 10);

        let outcome = transaction.apply_gas_price_ceiling(&mut bid);

        assert_eq!(outcome, GasPriceCeilingOutcome::WithinCeiling);
        assert!(!transaction.gas_price_ceiling_hit);
        assert_eq!(bid.max_fee.into_u128(), 120);
    }

    #[test]
    fn ceiling_hit_stays_false_while_bids_are_within_the_ceiling() {
        let mut transaction = test_transaction(0, TransactionStatus::PENDING);
        transaction.gas_price_ceiling = Some(ceiling(100, GasPriceCeilingBehavior::Freeze));
        let mut bid = test_gas_price(90, 5);

        let outcome = transaction.apply_gas_price_ceiling(&mut bid);

        assert_eq!(outcome, GasPriceCeilingOutcome::WithinCeiling);
        assert!(!transaction.gas_price_ceiling_hit);
    }

    #[test]
    fn ceiling_hit_is_recorded_when_freeze_blocks_a_bump() {
        let mut transaction = test_transaction(0, TransactionStatus::INMEMPOOL);
        transaction.gas_price_ceiling = Some(ceiling(100, GasPriceCeilingBehavior::Freeze));
        transaction.sent_with_gas = Some(test_gas_price(95, 5));
        let mut bid = test_gas_price(120, 10);

        let outcome = transaction.apply_gas_price_ceiling(&mut bid);

        assert_eq!(outcome, GasPriceCeilingOutcome::BlockedByCeiling);
        assert!(transaction.gas_price_ceiling_hit);
    }

    #[test]
    fn ceiling_hit_is_recorded_when_cap_clamps_a_bid() {
        let mut transaction = test_transaction(0, TransactionStatus::INMEMPOOL);
        transaction.gas_price_ceiling = Some(ceiling(100, GasPriceCeilingBehavior::Cap));
        transaction.sent_with_gas = Some(test_gas_price(95, 5));
        let mut bid = test_gas_price(120, 10);

        let outcome = transaction.apply_gas_price_ceiling(&mut bid);

        assert_eq!(outcome, GasPriceCeilingOutcome::ClampedToCeiling);
        assert!(transaction.gas_price_ceiling_hit);
        assert_eq!(bid.max_fee.into_u128(), 100);
    }

    #[test]
    fn ceiling_hit_is_surfaced_in_the_serialized_transaction() {
        let mut transaction = test_transaction(0, TransactionStatus::EXPIRED);
        transaction.gas_price_ceiling_hit = true;

        let serialized =
            serde_json::to_value(&transaction).expect("transaction should serialize to json");

        assert_eq!(serialized["gasPriceCeilingHit"], serde_json::Value::Bool(true));
    }
}
