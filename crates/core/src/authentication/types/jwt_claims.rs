use serde::{Deserialize, Serialize};

use super::JwtRole;
use crate::shared::common_types::EvmAddress;

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: EvmAddress,
    pub role: JwtRole,
    exp: usize,
    iat: usize,
}

impl JwtClaims {
    /// Creates a new JWT claims instance.
    ///
    /// Constructs a new JwtClaims struct with the provided subject (EVM address),
    /// role, expiration time, and issued at time.
    ///
    /// # Arguments
    /// * `sub` - The subject (EVM address) that the token represents
    /// * `role` - The role/permissions level for the user
    /// * `exp` - The expiration time as a Unix timestamp in seconds
    /// * `iat` - The issued at time as a Unix timestamp in seconds
    ///
    /// # Returns
    /// * `Self` - A new JwtClaims instance with the provided values
    pub fn new(sub: EvmAddress, role: JwtRole, exp: usize, iat: usize) -> Self {
        Self { sub, role, exp, iat }
    }
}
