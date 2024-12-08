use std::{
    error::Error,
    hash::{Hash, Hasher},
    str,
    str::FromStr,
};

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio_postgres::types::{FromSql, IsNull, ToSql, Type};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq)]
pub struct GasPrice(pub u128);

impl GasPrice {
    pub fn new(gas_price: u128) -> Self {
        GasPrice(gas_price)
    }
}

impl Hash for GasPrice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for GasPrice {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<'a> FromSql<'a> for GasPrice {
    fn from_sql(_ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let s = str::from_utf8(raw)?;
        let value = s.parse::<u128>()?;
        Ok(GasPrice(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::TEXT || *ty == Type::CHAR || *ty == Type::VARCHAR || *ty == Type::BPCHAR
    }
}

impl ToSql for GasPrice {
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
pub struct ParseGasPriceError;

impl std::fmt::Display for ParseGasPriceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid gas price")
    }
}

impl Error for ParseGasPriceError {}

impl FromStr for GasPrice {
    type Err = ParseGasPriceError;

    fn from_str(param: &str) -> Result<Self, Self::Err> {
        param.parse::<u128>().map(GasPrice).map_err(|_| ParseGasPriceError)
    }
}

impl From<GasPrice> for u128 {
    fn from(gas_price: GasPrice) -> Self {
        gas_price.0
    }
}

impl From<u128> for GasPrice {
    fn from(gas_price: u128) -> Self {
        GasPrice(gas_price)
    }
}
