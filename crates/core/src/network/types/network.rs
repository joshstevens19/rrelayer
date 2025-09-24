use serde::{Deserialize, Serialize};

use super::ChainId;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Network {
    pub name: String,
    #[serde(rename = "chainId")]
    pub chain_id: ChainId,
    #[serde(rename = "providerUrls")]
    pub provider_urls: Vec<String>,
}
