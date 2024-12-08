use std::{error::Error, fmt::Display, str::FromStr};

use alloy::primitives::Address;
use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, PartialEq, Deserialize)]
pub struct EvmAddress(Address);

impl EvmAddress {
    pub fn hex(&self) -> String {
        format!("{:?}", self.0)
    }

    pub fn new(address: Address) -> Self {
        EvmAddress(address)
    }
}

impl Display for EvmAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", self.0)
    }
}

impl<'a> FromSql<'a> for EvmAddress {
    fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let s = String::from_utf8(raw.to_vec())?;

        if s.len() == 42 {
            Ok(EvmAddress(s.parse::<Address>()?))
        } else {
            Err("Invalid length for EVM address".into())
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}

impl ToSql for EvmAddress {
    fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let address_str = self.hex();

        out.extend_from_slice(address_str.as_bytes());

        Ok(IsNull::No)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }

    tokio_postgres::types::to_sql_checked!();
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
