use serde::{Deserialize, Serialize};

use super::{AccessToken, RefreshToken};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPair {
    #[serde(rename = "accessToken")]
    pub access_token: AccessToken,

    #[serde(rename = "refreshToken")]
    pub refresh_token: RefreshToken,
}
