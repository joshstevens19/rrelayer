use alloy::primitives::{utils::UnitsError, ParseSignedError};
use async_trait::async_trait;
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
pub struct TenderlyGasProviderSetupConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TenderlyGasEstimateSpeedResult {
    #[serde(rename = "maxPriorityFeePerGas")]
    max_priority_fee_per_gas: String,

    #[serde(rename = "maxFeePerGas")]
    max_fee_per_gas: String,

    #[serde(rename = "waitTime")]
    wait_time: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TenderlyGasEstimatePriceResult {
    low: TenderlyGasEstimateSpeedResult,

    medium: TenderlyGasEstimateSpeedResult,

    high: TenderlyGasEstimateSpeedResult,
}

impl TenderlyGasEstimatePriceResult {
    fn gas_price_result(
        speed: &TenderlyGasEstimateSpeedResult,
        is_super_fast: bool,
    ) -> Result<GasPriceResult, UnitsError> {
        let (priority_multiplier, wait_multiplier) = if is_super_fast {
            (120, 80) // 120% for fees, 80% for wait times
        } else {
            (100, 100) // No adjustment for other speeds
        };

        let max_priority_fee = parse_formatted_gas_to_u128(&speed.max_priority_fee_per_gas)?
            .checked_mul(priority_multiplier)
            .and_then(|v| v.checked_div(100))
            .ok_or(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))?;

        let max_fee = parse_formatted_gas_to_u128(&speed.max_fee_per_gas)?
            .checked_mul(priority_multiplier)
            .and_then(|v| v.checked_div(100))
            .ok_or(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))?;

        Ok(GasPriceResult {
            max_priority_fee: MaxPriorityFee::new(max_priority_fee),
            max_fee: MaxFee::new(max_fee),
            min_wait_time_estimate: Some(speed.wait_time * wait_multiplier / 100),
            max_wait_time_estimate: Some(speed.wait_time * wait_multiplier / 100),
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
pub struct TenderlyGasFeeChainConfig {
    rpc_url: String,
    chain_id: ChainId,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TenderlyGasFeeEstimator {
    supported_chains: Vec<TenderlyGasFeeChainConfig>,
    api_key: String,
}

#[derive(Debug, Deserialize)]
struct TenderlyGasEstimateJsonRpcResult {
    result: TenderlyGasEstimateResult,
}

#[derive(Debug, Deserialize)]
struct TenderlyGasEstimateResult {
    // current_block_number: String,
    // base_fee_per_gas: String,
    price: TenderlyGasEstimatePriceResult,
}

impl TenderlyGasFeeEstimator {
    pub fn new(api_key: &str) -> Self {
        let supported_chains = vec![
            TenderlyGasFeeChainConfig {
                rpc_url: "https://mainnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(11155111),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://holesky.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(17000),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://arbitrum.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(42161),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://arbitrum-nova.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(42170),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://arbitrum-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(421614),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://avalanche.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(43114),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://avalanche-fuji.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(43113),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://linea.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(59144),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://linea-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(59141),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://base.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(8453),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://base-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(84532),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://optimism.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(10),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://optimism-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(11155420),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://polygon.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(137),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://polygon-amoy.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(80002),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://apechain.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(33139),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://curtis.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(33111),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://corn.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(21000000),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://corn-testnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(21000001),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://blast.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(81457),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://boba-bnb.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(56288),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://boba-bnb-testnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(9728),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://boba-ethereum.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(288),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://boba-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(28882),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://berachain.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(80094),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://concrete.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(12739),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://concrete-testnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(18291),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://flare.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(14),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://fraxtal.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(252),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://fraxtal-holesky.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(2522),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://bob.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(60808),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://bob-testnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(808813),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://immutable.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(13371),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://immutable-testnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(13473),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://ink.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(57073),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://ink-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(763373),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://kinto.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(7887),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://lisk.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1135),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://lens.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(232),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://lens-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(37111),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://lisk-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(4202),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://mantle.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(5000),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://mantle-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(5003),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://metis-andromeda.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1088),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://metis-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(59902),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://mode.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(34443),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://mode-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(919),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://morph.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(2818),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://morph-holesky.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(2810),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://polynomial.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(8008),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://polynomial-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(80008),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://tangible-real.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(111188),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://tangible-unreal.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(18233),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://soneium.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1868),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://soneium-minato.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1946),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://sonic.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(146),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://sonic-blaze.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(57054),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://swellchain.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1923),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://swellchain-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1924),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://taiko-hekla.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(167009),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://taiko-mainnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(167000),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://unichain.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(130),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://unichain-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1301),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://worldchain-mainnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(480),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://worldchain-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(4801),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://scroll-mainnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(534352),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://scroll-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(534351),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://ronin.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(2020),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://ronin-testnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(2021),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://sophon.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(50104),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://sophon-testnet.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(53105104),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://story.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1514),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://story-aeneid.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(1315),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://zksync.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(324),
            },
            TenderlyGasFeeChainConfig {
                rpc_url: "https://zksync-sepolia.gateway.tenderly.co".to_string(),
                chain_id: ChainId::new(300),
            },
        ];

        Self { supported_chains, api_key: api_key.to_string() }
    }

    fn build_suggested_gas_price_endpoint(&self, chain_id: &ChainId) -> Result<String, String> {
        let rpc_url = self
            .supported_chains
            .iter()
            .find(|c| c.chain_id == *chain_id)
            .ok_or_else(|| format!("Chain not found: {}", chain_id))?
            .rpc_url
            .clone();
        Ok(format!("{}/{}", rpc_url, self.api_key))
    }

    async fn request_gas_estimate(
        &self,
        chain_id: &ChainId,
    ) -> Result<TenderlyGasEstimatePriceResult, reqwest::Error> {
        let url = match self.build_suggested_gas_price_endpoint(chain_id) {
            Ok(url) => url,
            Err(_) => {
                let client = reqwest::Client::new();
                let result = client.get("http://").send().await;
                match result {
                    Err(error) => return Err(error),
                    Ok(_) => unreachable!("This should always fail"),
                }
            }
        };
        println!("Tenderly gas estimate url: {}", url);
        let client = reqwest::Client::new();

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tenderly_gasPrice",
            "params": []
        });
        println!("Tenderly gas estimate body: {:?}", body);

        let gas_estimate_result: TenderlyGasEstimateJsonRpcResult =
            client.post(url).json(&body).send().await?.json().await?;

        Ok(gas_estimate_result.result.price)
    }
}

#[async_trait]
impl BaseGasFeeEstimator for TenderlyGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError> {
        let gas_estimate_result = self.request_gas_estimate(chain_id).await?;

        Ok(gas_estimate_result.to_base_result().map_err(GasEstimatorError::UnitsError)?)
    }

    fn is_chain_supported(&self, chain_id: &ChainId) -> bool {
        self.supported_chains.iter().any(|config| config.chain_id == *chain_id)
    }
}
