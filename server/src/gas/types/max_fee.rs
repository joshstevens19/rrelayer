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
pub struct MaxFee(u128);

impl MaxFee {
    pub fn new(value: u128) -> Self {
        MaxFee(value)
    }

    pub fn into_u128(self) -> u128 {
        self.0
    }
}

impl Hash for MaxFee {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for MaxFee {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Mul<u32> for MaxFee {
    type Output = MaxFee;

    fn mul(self, other: u32) -> Self::Output {
        MaxFee(self.0 * other as u128)
    }
}

impl Div<u32> for MaxFee {
    type Output = MaxFee;

    fn div(self, other: u32) -> Self::Output {
        MaxFee(self.0 / other as u128)
    }
}

impl Add<MaxFee> for MaxFee {
    type Output = MaxFee;

    fn add(self, other: MaxFee) -> MaxFee {
        MaxFee(self.0 + other.0)
    }
}

impl Add<u128> for MaxFee {
    type Output = MaxFee;

    fn add(self, other: u128) -> MaxFee {
        MaxFee(self.0 + other)
    }
}

impl<'a> FromSql<'a> for MaxFee {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let s = str::from_utf8(raw)?;
        let value = s.parse::<u128>()?;
        Ok(MaxFee(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}

impl ToSql for MaxFee {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let value_str = self.0.to_string();
        value_str.to_sql(ty, out)
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }

    tokio_postgres::types::to_sql_checked!();
}

#[derive(Debug)]
pub struct ParseMaxFeeError;

impl std::fmt::Display for ParseMaxFeeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid max fee")
    }
}

impl Error for ParseMaxFeeError {}

impl FromStr for MaxFee {
    type Err = ParseMaxFeeError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        param.parse::<u128>().map(MaxFee).map_err(|_| ParseMaxFeeError)
    }
}

impl From<u128> for MaxFee {
    fn from(max_fee: u128) -> Self {
        MaxFee(max_fee)
    }
}

impl From<MaxFee> for u128 {
    fn from(max_fee: MaxFee) -> Self {
        max_fee.0
    }
}
