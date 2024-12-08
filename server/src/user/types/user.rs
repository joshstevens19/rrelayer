use serde::{Deserialize, Serialize};

use crate::{authentication::types::JwtRole, shared::common_types::EvmAddress};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct User {
    pub address: EvmAddress,
    pub role: JwtRole,
}
