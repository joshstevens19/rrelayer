use std::{error::Error, str::from_utf8};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

use crate::gas::{GasPrice, GasPriceResult, MaxFee, MaxPriorityFee};

/// How the queue reacts when a freshly computed bid for a transaction would exceed its
/// [`GasPriceCeiling`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GasPriceCeilingBehavior {
    /// Never bid above the ceiling: a bump that would exceed it is skipped and the last
    /// compliant bid stays live until it mines or the transaction expires through the
    /// normal no-op machinery. A FIRST bid already above the ceiling is rejected at
    /// queue admission (and held back at broadcast time if the market moved after
    /// queueing) rather than dishonestly broadcast above the ceiling.
    #[default]
    Freeze,
    /// Clamp the bid at exactly the ceiling - including the first bid - and keep it
    /// live until the transaction mines or expires.
    Cap,
}

impl GasPriceCeilingBehavior {
    fn as_str(&self) -> &'static str {
        match self {
            GasPriceCeilingBehavior::Freeze => "freeze",
            GasPriceCeilingBehavior::Cap => "cap",
        }
    }
}

impl<'a> FromSql<'a> for GasPriceCeilingBehavior {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        if !<Self as FromSql>::accepts(ty) {
            return Err(format!("Unexpected type for GasPriceCeilingBehavior: {}", ty).into());
        }

        let behavior = from_utf8(raw).map_err(|err| format!("Invalid UTF-8 sequence: {}", err))?;

        match behavior {
            "freeze" => Ok(GasPriceCeilingBehavior::Freeze),
            "cap" => Ok(GasPriceCeilingBehavior::Cap),
            _ => Err(format!("Unknown GasPriceCeilingBehavior: {}", behavior).into()),
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}

impl ToSql for GasPriceCeilingBehavior {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        if !<Self as ToSql>::accepts(ty) {
            return Err(format!("Unexpected type for GasPriceCeilingBehavior: {}", ty).into());
        }

        out.extend_from_slice(self.as_str().as_bytes());

        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }

    tokio_postgres::types::to_sql_checked!();
}

/// An absolute per-transaction gas price ceiling honored on the initial send and through
/// the gas bump loop. It bounds the EIP-1559 `max_fee_per_gas`, and because the legacy
/// gas price is derived from the max fee it bounds legacy transactions identically; blob
/// gas is not covered. The expiry close-out no-op is exempt: it must be able to
/// broadcast at market price or the reserved nonce would wedge the relayer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GasPriceCeiling {
    /// The absolute maximum price per gas (wei) this transaction may ever bid.
    #[serde(rename = "maxPrice")]
    pub max_price: GasPrice,

    /// What to do when a bid would exceed the ceiling - defaults to freeze.
    #[serde(default)]
    pub behavior: GasPriceCeilingBehavior,
}

/// Result of applying a [`GasPriceCeiling`] to a freshly computed bid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GasPriceCeilingOutcome {
    /// No ceiling configured or the bid is within it - send the bid as computed.
    WithinCeiling,
    /// Cap behavior clamped the bid to exactly the ceiling - send the clamped bid.
    ClampedToCeiling,
    /// The ceiling blocked this bid entirely: freeze semantics, or cap when the ceiling
    /// bid is already live in the mempool. Nothing should be broadcast.
    BlockedByCeiling,
}

impl GasPriceCeiling {
    /// Applies this ceiling to a freshly computed bid, clamping it in place for cap
    /// behavior. `previously_sent_with` is the bid currently live in the mempool
    /// (`None` for a first send).
    pub fn apply(
        &self,
        gas_price: &mut GasPriceResult,
        previously_sent_with: Option<&GasPriceResult>,
    ) -> GasPriceCeilingOutcome {
        let ceiling = self.max_price.into_u128();
        if gas_price.max_fee.into_u128() <= ceiling {
            return GasPriceCeilingOutcome::WithinCeiling;
        }

        match self.behavior {
            GasPriceCeilingBehavior::Freeze => GasPriceCeilingOutcome::BlockedByCeiling,
            GasPriceCeilingBehavior::Cap => {
                // A clamped bid can only replace a cheaper one: once the last broadcast
                // already bid the ceiling there is nothing higher to send, so keep that
                // bid live instead of re-signing an identical payload.
                if let Some(sent) = previously_sent_with {
                    if sent.max_fee.into_u128() >= ceiling {
                        return GasPriceCeilingOutcome::BlockedByCeiling;
                    }
                }

                gas_price.max_fee = MaxFee::new(ceiling);
                if gas_price.max_priority_fee.into_u128() > ceiling {
                    gas_price.max_priority_fee = MaxPriorityFee::new(ceiling);
                }

                GasPriceCeilingOutcome::ClampedToCeiling
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gas_price(max_fee: u128, max_priority_fee: u128) -> GasPriceResult {
        GasPriceResult {
            max_fee: MaxFee::new(max_fee),
            max_priority_fee: MaxPriorityFee::new(max_priority_fee),
            min_wait_time_estimate: None,
            max_wait_time_estimate: None,
        }
    }

    fn ceiling(max_price: u128, behavior: GasPriceCeilingBehavior) -> GasPriceCeiling {
        GasPriceCeiling { max_price: GasPrice::new(max_price), behavior }
    }

    #[test]
    fn bid_within_ceiling_is_untouched() {
        for behavior in [GasPriceCeilingBehavior::Freeze, GasPriceCeilingBehavior::Cap] {
            let mut bid = gas_price(90, 5);
            let outcome = ceiling(100, behavior).apply(&mut bid, None);

            assert_eq!(outcome, GasPriceCeilingOutcome::WithinCeiling);
            assert_eq!(bid.max_fee.into_u128(), 90);
            assert_eq!(bid.max_priority_fee.into_u128(), 5);
        }
    }

    #[test]
    fn freeze_blocks_a_bump_above_the_ceiling_and_keeps_the_bid_unchanged() {
        let previous = gas_price(95, 5);
        let mut bid = gas_price(120, 10);
        let outcome =
            ceiling(100, GasPriceCeilingBehavior::Freeze).apply(&mut bid, Some(&previous));

        assert_eq!(outcome, GasPriceCeilingOutcome::BlockedByCeiling);
        // The candidate bid must not be mutated - the caller keeps the last compliant bid
        assert_eq!(bid.max_fee.into_u128(), 120);
    }

    #[test]
    fn cap_clamps_a_bump_to_exactly_the_ceiling() {
        let previous = gas_price(95, 5);
        let mut bid = gas_price(120, 10);
        let outcome = ceiling(100, GasPriceCeilingBehavior::Cap).apply(&mut bid, Some(&previous));

        assert_eq!(outcome, GasPriceCeilingOutcome::ClampedToCeiling);
        assert_eq!(bid.max_fee.into_u128(), 100);
        assert_eq!(bid.max_priority_fee.into_u128(), 10);
    }

    #[test]
    fn cap_clamps_the_priority_fee_down_to_the_ceiling() {
        let mut bid = gas_price(120, 110);
        let outcome = ceiling(100, GasPriceCeilingBehavior::Cap).apply(&mut bid, None);

        assert_eq!(outcome, GasPriceCeilingOutcome::ClampedToCeiling);
        assert_eq!(bid.max_fee.into_u128(), 100);
        assert_eq!(bid.max_priority_fee.into_u128(), 100);
    }

    #[test]
    fn cap_blocks_rebroadcast_once_the_ceiling_bid_is_live() {
        let previous = gas_price(100, 10);
        let mut bid = gas_price(120, 12);
        let outcome = ceiling(100, GasPriceCeilingBehavior::Cap).apply(&mut bid, Some(&previous));

        assert_eq!(outcome, GasPriceCeilingOutcome::BlockedByCeiling);
        assert_eq!(bid.max_fee.into_u128(), 120);
    }

    #[test]
    fn freeze_blocks_a_first_bid_above_the_ceiling() {
        let mut bid = gas_price(120, 10);
        let outcome = ceiling(100, GasPriceCeilingBehavior::Freeze).apply(&mut bid, None);

        assert_eq!(outcome, GasPriceCeilingOutcome::BlockedByCeiling);
    }

    #[test]
    fn cap_clamps_a_first_bid_above_the_ceiling() {
        let mut bid = gas_price(120, 10);
        let outcome = ceiling(100, GasPriceCeilingBehavior::Cap).apply(&mut bid, None);

        assert_eq!(outcome, GasPriceCeilingOutcome::ClampedToCeiling);
        assert_eq!(bid.max_fee.into_u128(), 100);
    }
}
