use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum GasProvider {
    Infura,
    Tenderly,
    Custom,
}

#[derive(Debug, Clone)]
pub struct ConversionError {
    pub message: String,
}

impl FromStr for GasProvider {
    type Err = ConversionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "infura" => Ok(GasProvider::Infura),
            "tenderly" => Ok(GasProvider::Tenderly),
            "custom" => Ok(GasProvider::Custom),
            // Add other cases as needed
            _ => Err(ConversionError { message: format!("Unsupported gas provider: {}", s) }),
        }
    }
}

/// Custom deserializer for GasProvider that handles optional string values.
///
/// Converts string representations to GasProvider enum variants, supporting
/// case-insensitive matching.
///
/// # Arguments
/// * `deserializer` - The serde deserializer
///
/// # Returns
/// * `Ok(Some(GasProvider))` - Successfully parsed gas provider
/// * `Ok(None)` - If the field was not present
/// * `Err(D::Error)` - If parsing failed
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
