use std::{error::Error, fmt::Display, str::FromStr};

use alloy::primitives::Address;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{to_sql_checked, FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Eq, Hash, Deserialize)]
pub struct EvmAddress(Address);

impl EvmAddress {
    /// Returns the hexadecimal string representation of the address.
    ///
    /// # Returns
    /// * `String` - The address formatted as a hex string with 0x prefix
    pub fn hex(&self) -> String {
        format!("{:?}", self.0)
    }

    /// Creates a new EvmAddress wrapper around an Alloy Address.
    ///
    /// # Arguments
    /// * `address` - The Alloy Address to wrap
    ///
    /// # Returns
    /// * `Self` - A new EvmAddress instance
    pub fn new(address: Address) -> Self {
        EvmAddress(address)
    }

    /// Consumes this EvmAddress and returns the inner Alloy Address.
    ///
    /// # Returns
    /// * `Address` - The inner Alloy Address
    pub fn into_address(self) -> Address {
        self.0
    }

    pub fn dead() -> Self {
        Self::zero()
    }

    // Just a less scary alias for `dead` address.
    pub fn zero() -> Self {
        Self(Address::ZERO)
    }
}

impl Display for EvmAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<'a> FromSql<'a> for EvmAddress {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        // Ensure the byte slice is the correct length for an Ethereum address (20 bytes)
        if raw.len() != 20 {
            return Err("Invalid byte length for Ethereum address".into());
        }

        let address = Address::from_slice(raw);

        Ok(EvmAddress(address))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }
}

impl ToSql for EvmAddress {
    fn to_sql(
        &self,
        _ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        out.extend_from_slice(self.into_address().as_slice());
        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::BYTEA
    }

    to_sql_checked!();
}

#[derive(Debug)]
pub struct ParseEvmAddressError(String);

impl Display for ParseEvmAddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid EVM address: {}", self.0)
    }
}

impl Error for ParseEvmAddressError {}

impl FromStr for EvmAddress {
    type Err = ParseEvmAddressError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        Address::from_str(param).map(EvmAddress).map_err(|e| ParseEvmAddressError(e.to_string()))
    }
}

impl From<EvmAddress> for Address {
    fn from(address: EvmAddress) -> Self {
        address.0
    }
}

impl From<Address> for EvmAddress {
    fn from(address: Address) -> Self {
        EvmAddress(address)
    }
}
