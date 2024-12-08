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
    pub fn new(sub: EvmAddress, role: JwtRole, exp: usize, iat: usize) -> Self {
        Self { sub, role, exp, iat }
    }
}
