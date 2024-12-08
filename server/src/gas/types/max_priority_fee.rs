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
pub struct MaxPriorityFee(pub u128);

impl MaxPriorityFee {
    pub fn new(value: u128) -> Self {
        MaxPriorityFee(value)
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
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let s = str::from_utf8(raw)?;
        let value = s.parse::<u128>()?;
        Ok(MaxPriorityFee(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::INT4 || *ty == Type::VARCHAR
    }
}

impl ToSql for MaxPriorityFee {
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
