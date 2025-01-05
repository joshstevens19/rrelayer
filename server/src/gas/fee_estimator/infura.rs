use alloy::primitives::{utils::UnitsError, ParseSignedError};
use async_trait::async_trait;
use base64::engine::{general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};

use super::base::{
    parse_formatted_gas_to_u128, BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult,
    GasPriceResult,
};
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::types::ChainId,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct InfuraGasProviderSetupConfig {
    pub api_key: String,
    pub secret: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct InfuraGasEstimateSpeedResult {
    #[serde(rename = "suggestedMaxPriorityFeePerGas")]
    suggested_max_priority_fee_per_gas: String,

    #[serde(rename = "suggestedMaxFeePerGas")]
    suggested_max_fee_per_gas: String,

    #[serde(rename = "minWaitTimeEstimate")]
    min_wait_time_estimate: i64,

    #[serde(rename = "maxWaitTimeEstimate")]
    max_wait_time_estimate: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct InfuraGasEstimateResult {
    low: InfuraGasEstimateSpeedResult,

    medium: InfuraGasEstimateSpeedResult,

    high: InfuraGasEstimateSpeedResult,
}

impl InfuraGasEstimateResult {
    fn gas_price_result(
        speed: &InfuraGasEstimateSpeedResult,
        is_super_fast: bool,
    ) -> Result<GasPriceResult, UnitsError> {
        let (priority_multiplier, wait_multiplier) = if is_super_fast {
            (120, 80) // 120% for fees, 80% for wait times
        } else {
            (100, 100) // No adjustment for other speeds
        };

        let max_priority_fee =
            parse_formatted_gas_to_u128(&speed.suggested_max_priority_fee_per_gas)?
                .checked_mul(priority_multiplier)
                .and_then(|v| v.checked_div(100))
                .ok_or(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))?;

        let max_fee = parse_formatted_gas_to_u128(&speed.suggested_max_fee_per_gas)?
            .checked_mul(priority_multiplier)
            .and_then(|v| v.checked_div(100))
            .ok_or(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))?;

        Ok(GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(max_priority_fee),
            max_fee: MaxFee::new(max_fee),
            min_wait_time_estimate: Some(speed.min_wait_time_estimate * wait_multiplier / 100),
            max_wait_time_estimate: Some(speed.max_wait_time_estimate * wait_multiplier / 100),
        })
    }

    pub fn to_base_result(&self) -> Result<GasEstimatorResult, UnitsError> {
        Ok(GasEstimatorResult {
            slow: Self::gas_price_result(&self.low, false)?,
            medium: Self::gas_price_result(&self.medium, false)?,
            fast: Self::gas_price_result(&self.high, false)?,
            super_fast: Self::gas_price_result(&self.high, true)?,
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct InfuraGasFeeEstimator {
    base_url: String,
    supported_chains: Vec<ChainId>,
    api_key: String,
    secret: String,
}

impl InfuraGasFeeEstimator {
    pub fn new(api_key: &str, secret: &str) -> Self {
        Self {
            base_url: "https://gas.api.infura.io/networks".to_string(),
            supported_chains: vec![
                ChainId(1),        // EthereumMainnet
                ChainId(5),        // EthereumGoerli
                ChainId(11155111), // EthereumSepolia
                ChainId(42161),    // ArbitrumMainnet
                ChainId(42170),    // ArbitrumNova
                ChainId(43114),    // Avalanche
                ChainId(8453),     // Base
                ChainId(56),       // Binance
                ChainId(204),      // OpBnbLayer2
                ChainId(25),       // Cronos
                ChainId(250),      // Fantom
                ChainId(314),      // Filecoin
                ChainId(59144),    // LineaMainnet
                ChainId(59140),    // LineaTestnet
                ChainId(10),       // Optimism
                ChainId(137),      // PolygonMainnet
                ChainId(100),      // PolygonMumbai
                ChainId(324),      // ZkSyncEraMainnet
            ],
            api_key: api_key.to_string(),
            secret: secret.to_string(),
        }
    }

    fn build_suggested_gas_price_endpoint(&self, chain_id: &ChainId) -> String {
        format!("{}/{}/suggestedGasFees", self.base_url, chain_id)
    }

    async fn request_gas_estimate(
        &self,
        chain_id: &ChainId,
    ) -> Result<InfuraGasEstimateResult, reqwest::Error> {
        let url = self.build_suggested_gas_price_endpoint(chain_id);
        let client = reqwest::Client::new();

        let credentials = format!("{}:{}", self.api_key, self.secret);
        let encoded_credentials = BASE64.encode(credentials);
        let auth_header = format!("Basic {}", encoded_credentials);

        let gas_estimate_result: InfuraGasEstimateResult = client
            .get(url)
            .header("Accept", "application/json")
            .header("Authorization", auth_header)
            .send()
            .await? // Await the response
            .json()
            .await?;

        Ok(gas_estimate_result)
    }
}

#[async_trait]
impl BaseGasFeeEstimator for InfuraGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let gas_estimate_result = self.request_gas_estimate(chain_id).await?;

        Ok(gas_estimate_result.to_base_result().map_err(GasEstimatorError::UnitsError)?)
    }

    fn is_chain_supported(&self, chain_id: &ChainId) -> bool {
        self.supported_chains.iter().any(|&id| chain_id == &id)
    }
}
