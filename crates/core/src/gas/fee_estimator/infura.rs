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
    network::ChainId,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InfuraGasProviderSetupConfig {
    pub enabled: bool,
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
    /// Converts an Infura gas estimate speed result to the standard gas price result format.
    ///
    /// # Arguments
    /// * `speed` - The Infura gas estimate data for a specific speed
    /// * `is_super_fast` - Whether this is for the super fast tier (applies 120% multiplier)
    ///
    /// # Returns
    /// * `Ok(GasPriceResult)` - The converted standard gas price result
    /// * `Err(UnitsError)` - If parsing the gas price strings fails or overflow occurs
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

    /// Converts the Infura gas estimate result to the standard gas estimator result format.
    ///
    /// Maps Infura's low/medium/high speeds to slow/medium/fast, and creates super_fast
    /// by applying a multiplier to the high speed estimates.
    ///
    /// # Returns
    /// * `Ok(GasEstimatorResult)` - The converted standard gas estimator result
    /// * `Err(UnitsError)` - If parsing any of the gas price strings fails
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
    /// Creates a new Infura gas fee estimator with API credentials and supported chains.
    ///
    /// # Arguments
    /// * `api_key` - The Infura API key for authentication
    /// * `secret` - The Infura API secret for authentication
    ///
    /// # Returns
    /// * A new `InfuraGasFeeEstimator` instance configured with all supported chains
    pub fn new(api_key: &str, secret: &str) -> Self {
        Self {
            base_url: "https://gas.api.infura.io/networks".to_string(),
            supported_chains: vec![
                ChainId::new(1),           // EthereumMainnet
                ChainId::new(5),           // EthereumGoerli
                ChainId::new(11155111),    // EthereumSepolia
                ChainId::new(42161),       // ArbitrumMainnet
                ChainId::new(42170),       // ArbitrumNova
                ChainId::new(43114),       // Avalanche
                ChainId::new(8453),        // Base
                ChainId::new(56),          // Binance
                ChainId::new(204),         // OpBnbLayer2
                ChainId::new(25),          // Cronos
                ChainId::new(250),         // Fantom
                ChainId::new(314),         // Filecoin
                ChainId::new(59144),       // LineaMainnet
                ChainId::new(59140),       // LineaTestnet
                ChainId::new(10),          // Optimism
                ChainId::new(137),         // PolygonMainnet
                ChainId::new(100),         // PolygonMumbai
                ChainId::new(324),         // ZkSyncEraMainnet
                ChainId::new(5000),        // Mantle
                ChainId::new(11297108109), // Palm
                ChainId::new(534352),      // Scroll
                ChainId::new(1923),        // Swellchain
                ChainId::new(130),         // Unichain
            ],
            api_key: api_key.to_string(),
            secret: secret.to_string(),
        }
    }

    /// Builds the Infura API endpoint URL for requesting gas prices for a specific chain.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to build the endpoint for
    ///
    /// # Returns
    /// * `String` - The complete Infura endpoint URL
    fn build_suggested_gas_price_endpoint(&self, chain_id: &ChainId) -> String {
        format!("{}/{}/suggestedGasFees", self.base_url, chain_id)
    }

    /// Requests gas price estimates from the Infura API for a specific chain.
    ///
    /// Uses HTTP Basic authentication with base64-encoded credentials.
    ///
    /// # Arguments
    /// * `chain_id` - The chain ID to request gas prices for
    ///
    /// # Returns
    /// * `Ok(InfuraGasEstimateResult)` - The gas estimates from Infura's API
    /// * `Err(reqwest::Error)` - If the HTTP request fails
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
