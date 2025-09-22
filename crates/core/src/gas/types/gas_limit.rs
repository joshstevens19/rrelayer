use std::{
    error::Error,
    hash::{Hash, Hasher},
    ops::{Add, Div, Mul},
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

    pub fn into_inner(self) -> u128 {
        self.0
    }
}

impl PartialEq for GasLimit {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd for GasLimit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GasLimit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
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

impl Add for GasLimit {
    type Output = GasLimit;

    fn add(self, other: GasLimit) -> Self::Output {
        GasLimit(self.0 + other.0)
    }
}

impl<'a> FromSql<'a> for GasLimit {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let decimal = rust_decimal::Decimal::from_sql(ty, raw)?;
        let value_str = decimal.to_string();
        let value = u128::from_str(&value_str)
            .map_err(|e| format!("Failed to convert decimal to u128: {}", e))?;
        Ok(GasLimit(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}

impl ToSql for GasLimit {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match rust_decimal::Decimal::from_str(&self.0.to_string()) {
            Ok(decimal) => decimal.to_sql(ty, out),
            Err(e) => Err(format!("Failed to convert to decimal: {}", e).into()),
        }
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
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
