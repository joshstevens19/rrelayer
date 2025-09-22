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
pub struct GasPrice(u128);

impl GasPrice {
    pub fn new(gas_price: u128) -> Self {
        GasPrice(gas_price)
    }

    pub fn into_u128(self) -> u128 {
        self.0
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
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let decimal = rust_decimal::Decimal::from_sql(ty, raw)?;
        let value_str = decimal.to_string();
        let value = u128::from_str(&value_str)
            .map_err(|e| format!("Failed to convert decimal to u128: {}", e))?;
        Ok(GasPrice(value))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::NUMERIC
    }
}

impl ToSql for GasPrice {
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
