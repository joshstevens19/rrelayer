use std::{
    error::Error,
    hash::{Hash, Hasher},
    ops::{Div, Mul},
    str,
    str::FromStr,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct GasLimit(u128);

impl GasLimit {
    pub fn new(gas_limit: u128) -> Self {
        GasLimit(gas_limit)
    }
}

impl PartialEq for GasLimit {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Hash for GasLimit {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Div<u32> for GasLimit {
    type Output = GasLimit;

    fn div(self, other: u32) -> Self::Output {
        GasLimit(self.0 / other as u128)
    }
}

impl Mul<u32> for GasLimit {
    type Output = GasLimit;

    fn mul(self, other: u32) -> Self::Output {
        GasLimit(self.0 * other as u128)
    }
}

impl<'a> FromSql<'a> for GasLimit {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let s = str::from_utf8(raw)?;
        let value = s.parse::<u128>()?;
        Ok(GasLimit(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}

impl ToSql for GasLimit {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let value_str = self.0.to_string();
        value_str.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        <String as ToSql>::accepts(ty)
    }

    tokio_postgres::types::to_sql_checked!();
}

#[derive(Debug)]
pub struct ParseGasLimitError;

impl std::fmt::Display for ParseGasLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid gas limit")
    }
}

impl Error for ParseGasLimitError {}

impl FromStr for GasLimit {
    type Err = ParseGasLimitError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        param.parse::<u128>().map(GasLimit).map_err(|_| ParseGasLimitError)
    }
}

impl From<GasLimit> for u128 {
    fn from(gas_limit: GasLimit) -> Self {
        gas_limit.0
    }
}

impl From<GasLimit> for u64 {
    fn from(gas_limit: GasLimit) -> Self {
        gas_limit.0 as u64
    }
}

impl From<u128> for GasLimit {
    fn from(gas_limit: u128) -> Self {
        GasLimit(gas_limit)
    }
}

impl From<u64> for GasLimit {
    fn from(gas_limit: u64) -> Self {
        GasLimit(gas_limit as u128)
    }
}
