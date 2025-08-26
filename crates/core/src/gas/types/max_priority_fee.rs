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

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialOrd)]
pub struct MaxPriorityFee(u128);

impl MaxPriorityFee {
    /// Creates a new MaxPriorityFee instance.
    ///
    /// # Arguments
    /// * `value` - The maximum priority fee value in wei
    ///
    /// # Returns
    /// * A new `MaxPriorityFee` instance
    pub fn new(value: u128) -> Self {
        MaxPriorityFee(value)
    }

    /// Extracts the inner u128 value from the MaxPriorityFee.
    ///
    /// # Returns
    /// * `u128` - The raw maximum priority fee value in wei
    pub fn into_u128(self) -> u128 {
        self.0
    }
}

impl PartialEq for MaxPriorityFee {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Hash for MaxPriorityFee {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Add<MaxPriorityFee> for MaxPriorityFee {
    type Output = MaxPriorityFee;

    fn add(self, other: MaxPriorityFee) -> MaxPriorityFee {
        MaxPriorityFee(self.0 + other.0)
    }
}

impl Mul<u32> for MaxPriorityFee {
    type Output = MaxPriorityFee;

    fn mul(self, other: u32) -> Self::Output {
        MaxPriorityFee(self.0 * other as u128)
    }
}

impl Div<u32> for MaxPriorityFee {
    type Output = MaxPriorityFee;

    fn div(self, other: u32) -> Self::Output {
        MaxPriorityFee(self.0 / other as u128)
    }
}

impl<'a> FromSql<'a> for MaxPriorityFee {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let decimal = rust_decimal::Decimal::from_sql(ty, raw)?;
        let value_str = decimal.to_string();
        let value = u128::from_str(&value_str)
            .map_err(|e| format!("Failed to convert decimal to u128: {}", e))?;
        Ok(MaxPriorityFee(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}

impl ToSql for MaxPriorityFee {
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
pub struct ParseMaxPriorityFeeError;

impl std::fmt::Display for ParseMaxPriorityFeeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid max priority fee")
    }
}

impl Error for ParseMaxPriorityFeeError {}

impl FromStr for MaxPriorityFee {
    type Err = ParseMaxPriorityFeeError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        param.parse::<u128>().map(MaxPriorityFee).map_err(|_| ParseMaxPriorityFeeError)
    }
}

impl From<MaxPriorityFee> for u128 {
    fn from(max_priority_fee: MaxPriorityFee) -> Self {
        max_priority_fee.0
    }
}

impl From<u128> for MaxPriorityFee {
    fn from(max_priority_fee: u128) -> Self {
        MaxPriorityFee(max_priority_fee)
    }
}
