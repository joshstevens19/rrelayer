use std::str::FromStr;

use alloy::primitives::U256;

/// Parses a string parameter into a U256 value.
pub fn from_param_u256(param: &str) -> Result<U256, &str> {
    match U256::from_str(param) {
        Ok(value) => Ok(value),
        Err(_) => Err("Failed to parse U256 from string"),
    }
}
