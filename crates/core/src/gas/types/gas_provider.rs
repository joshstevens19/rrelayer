use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GasProvider {
    BLOCKNATIVE,
    ETHERSCAN,
    INFURA,
    TENDERLY,
    CUSTOM,
}

#[derive(Debug, Clone)]
pub struct ConversionError {
    pub message: String,
}

impl FromStr for GasProvider {
    type Err = ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "BLOCKNATIVE" => Ok(GasProvider::BLOCKNATIVE),
            "ETHERSCAN" => Ok(GasProvider::ETHERSCAN),
            "INFURA" => Ok(GasProvider::INFURA),
            "TENDERLY" => Ok(GasProvider::TENDERLY),
            "CUSTOM" => Ok(GasProvider::CUSTOM),
            _ => Err(ConversionError { message: format!("Unsupported gas provider: {}", s) }),
        }
    }
}

pub fn deserialize_gas_provider<'de, D>(deserializer: D) -> Result<Option<GasProvider>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => {
            GasProvider::from_str(&s).map(Some).map_err(|e| serde::de::Error::custom(e.message))
        }
        None => Ok(None),
    }
}
