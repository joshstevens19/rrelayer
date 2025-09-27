use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::base::{BaseGasFeeEstimator, GasEstimatorError, GasEstimatorResult, GasPriceResult};
use crate::{
    gas::types::{MaxFee, MaxPriorityFee},
    network::ChainId,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EtherscanGasProviderSetupConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct EtherscanGasOracleResult {
    #[serde(rename = "SafeGasPrice")]
    safe_gas_price: String,
    #[serde(rename = "ProposeGasPrice")]
    propose_gas_price: String,
    #[serde(rename = "FastGasPrice")]
    fast_gas_price: String,
    #[serde(rename = "suggestBaseFee")]
    suggest_base_fee: String,
    #[serde(rename = "gasUsedRatio")]
    gas_used_ratio: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct EtherscanApiResponse {
    status: String,
    message: String,
    result: EtherscanGasOracleResult,
}

impl EtherscanGasOracleResult {
    fn parse_gwei_to_wei(gwei_str: &str) -> Result<u128, GasEstimatorError> {
        let gwei: f64 = gwei_str.parse().map_err(|_| {
            GasEstimatorError::CustomError(format!("Failed to parse gas price: {}", gwei_str))
        })?;

        // Convert Gwei to Wei (multiply by 10^9)
        Ok((gwei * 1_000_000_000.0) as u128)
    }

    pub fn to_base_result(&self) -> Result<GasEstimatorResult, GasEstimatorError> {
        // Parse base fee
        let base_fee = Self::parse_gwei_to_wei(&self.suggest_base_fee)?;

        // Parse gas prices (these are total gas prices, not just priority fees)
        let safe_total = Self::parse_gwei_to_wei(&self.safe_gas_price)?;
        let propose_total = Self::parse_gwei_to_wei(&self.propose_gas_price)?;
        let fast_total = Self::parse_gwei_to_wei(&self.fast_gas_price)?;

        // Calculate priority fees by subtracting base fee from total
        // Ensure priority fee is at least 1 gwei to avoid zero or negative values
        let min_priority_fee = 1_000_000_000u128; // 1 gwei

        let safe_priority = std::cmp::max(safe_total.saturating_sub(base_fee), min_priority_fee);
        let propose_priority =
            std::cmp::max(propose_total.saturating_sub(base_fee), min_priority_fee);
        let fast_priority = std::cmp::max(fast_total.saturating_sub(base_fee), min_priority_fee);

        // For super fast, add 20% buffer to fast
        let super_fast_priority = fast_priority * 120 / 100;
        let super_fast_total = fast_total * 120 / 100;

        let slow_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(safe_priority),
            max_fee: MaxFee::new(safe_total),
            min_wait_time_estimate: Some(300), // ~5 minutes for safe
            max_wait_time_estimate: Some(600), // ~10 minutes for safe
        };

        let medium_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(propose_priority),
            max_fee: MaxFee::new(propose_total),
            min_wait_time_estimate: Some(60), // ~1 minute for standard
            max_wait_time_estimate: Some(180), // ~3 minutes for standard
        };

        let fast_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(fast_priority),
            max_fee: MaxFee::new(fast_total),
            min_wait_time_estimate: Some(15), // ~15 seconds for fast
            max_wait_time_estimate: Some(60), // ~1 minute for fast
        };

        let super_fast_result = GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(super_fast_priority),
            max_fee: MaxFee::new(super_fast_total),
            min_wait_time_estimate: Some(5), // ~5 seconds for super fast
            max_wait_time_estimate: Some(15), // ~15 seconds for super fast
        };

        Ok(GasEstimatorResult {
            slow: slow_result,
            medium: medium_result,
            fast: fast_result,
            super_fast: super_fast_result,
        })
    }
}

pub struct EtherscanGasFeeEstimator {
    config: EtherscanGasProviderSetupConfig,
    client: reqwest::Client,
}

impl EtherscanGasFeeEstimator {
    pub fn new(config: EtherscanGasProviderSetupConfig) -> Result<Self, GasEstimatorError> {
        let client = reqwest::Client::new();
        Ok(Self { config, client })
    }

    fn get_api_base_url(&self, chain_id: &ChainId) -> Option<&'static str> {
        match chain_id.u64() {
            // Ethereum Mainnet
            1 => Some("https://api.etherscan.io"),
            // Ethereum Testnets
            11155111 => Some("https://api-sepolia.etherscan.io"), // Sepolia
            17000 => Some("https://api-holesky.etherscan.io"),    // Holesky

            // Polygon
            137 => Some("https://api.polygonscan.com"),
            80002 => Some("https://api-amoy.polygonscan.com"), // Polygon Amoy testnet

            // BSC (Binance Smart Chain)
            56 => Some("https://api.bscscan.com"),
            97 => Some("https://api-testnet.bscscan.com"), // BSC testnet

            // Optimism
            10 => Some("https://api-optimistic.etherscan.io"),
            11155420 => Some("https://api-sepolia-optimistic.etherscan.io"), // OP Sepolia

            // Arbitrum
            42161 => Some("https://api.arbiscan.io"),
            421614 => Some("https://api-sepolia.arbiscan.io"), // Arbitrum Sepolia

            // Base
            8453 => Some("https://api.basescan.org"),
            84532 => Some("https://api-sepolia.basescan.org"), // Base Sepolia

            // Avalanche
            43114 => Some("https://api.snowtrace.io"),
            43113 => Some("https://api-testnet.snowtrace.io"), // Avalanche Fuji testnet

            // Fantom
            250 => Some("https://api.ftmscan.com"),
            4002 => Some("https://api-testnet.ftmscan.com"), // Fantom testnet

            // Cronos
            25 => Some("https://api.cronoscan.com"),
            338 => Some("https://api-testnet.cronoscan.com"), // Cronos testnet

            // Moonbeam
            1284 => Some("https://api-moonbeam.moonscan.io"),
            1287 => Some("https://api-moonbase.moonscan.io"), // Moonbase Alpha testnet

            // Moonriver
            1285 => Some("https://api-moonriver.moonscan.io"),

            // Celo
            42220 => Some("https://api.celoscan.io"),
            44787 => Some("https://api-alfajores.celoscan.io"), // Celo Alfajores testnet

            // Gnosis Chain (formerly xDai)
            100 => Some("https://api.gnosisscan.io"),

            // Linea
            59144 => Some("https://api.lineascan.build"),
            59141 => Some("https://api-sepolia.lineascan.build"), // Linea Sepolia

            // Scroll
            534352 => Some("https://api.scrollscan.com"),
            534351 => Some("https://api-sepolia.scrollscan.com"), // Scroll Sepolia

            // Blast
            81457 => Some("https://api.blastscan.io"),
            168587773 => Some("https://api-sepolia.blastscan.io"), // Blast Sepolia

            // zkSync Era
            324 => Some("https://api-era.zksync.network"),
            300 => Some("https://api-sepolia.era.zksync.network"), // zkSync Sepolia

            // Mantle
            5000 => Some("https://api.mantlescan.xyz"),
            5003 => Some("https://api-sepolia.mantlescan.xyz"), // Mantle Sepolia

            // Fraxtal
            252 => Some("https://api.fraxscan.com"),
            2522 => Some("https://api-holesky.fraxscan.com"), // Fraxtal testnet

            // Mode
            34443 => Some("https://api.modescan.io"),
            919 => Some("https://api-sepolia.modescan.io"), // Mode Sepolia

            // Unsupported chains
            _ => None,
        }
    }
}

#[async_trait]
impl BaseGasFeeEstimator for EtherscanGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let base_url = self.get_api_base_url(chain_id).ok_or_else(|| {
            GasEstimatorError::CustomError(format!(
                "Etherscan API not supported for chain {}",
                chain_id.u64()
            ))
        })?;

        let url = format!(
            "{}/api?module=gastracker&action=gasoracle&apikey={}",
            base_url, self.config.api_key
        );

        let response =
            self.client.get(&url).send().await.map_err(|e| GasEstimatorError::ReqwestError(e))?;

        if !response.status().is_success() {
            return Err(GasEstimatorError::CustomError(format!(
                "Etherscan API returned status: {}",
                response.status()
            )));
        }

        let api_response: EtherscanApiResponse =
            response.json().await.map_err(|e| GasEstimatorError::CustomError(e.to_string()))?;

        if api_response.status != "1" {
            return Err(GasEstimatorError::CustomError(format!(
                "Etherscan API error: {}",
                api_response.message
            )));
        }

        api_response.result.to_base_result()
    }

    fn is_chain_supported(&self, chain_id: &ChainId) -> bool {
        self.get_api_base_url(chain_id).is_some()
    }
}
