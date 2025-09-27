use std::sync::Arc;

use alloy::primitives::{
    utils::{parse_units, ParseUnits, UnitsError},
    ParseSignedError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    blocknative::BlockNativeGasFeeEstimator, etherscan::EtherscanGasFeeEstimator,
    infura::InfuraGasFeeEstimator,
};
use crate::gas::fee_estimator::fallback::FallbackGasFeeEstimator;
use crate::{
    create_retry_client,
    gas::{
        fee_estimator::tenderly::TenderlyGasFeeEstimator,
        types::{GasPrice, GasProvider, MaxFee, MaxPriorityFee},
    },
    network::ChainId,
    provider::RetryClientError,
    NetworkSetupConfig, SetupConfig,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GasPriceResult {
    #[serde(rename = "maxPriorityFee")]
    pub max_priority_fee: MaxPriorityFee,

    #[serde(rename = "maxFee")]
    pub max_fee: MaxFee,

    #[serde(rename = "minWaitTimeEstimate")]
    pub min_wait_time_estimate: Option<i64>,

    #[serde(rename = "maxWaitTimeEstimate")]
    pub max_wait_time_estimate: Option<i64>,
}

impl GasPriceResult {
    /// Calculates the effective gas price for legacy transactions.
    ///
    /// For EIP-1559 transactions, the effective gas price is the sum of
    /// the base fee and priority fee (max fee per gas).
    pub fn legacy_gas_price(&self) -> GasPrice {
        GasPrice::new(self.max_fee.into_u128() + self.max_priority_fee.into_u128())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GasEstimatorResult {
    pub slow: GasPriceResult,

    pub medium: GasPriceResult,

    pub fast: GasPriceResult,

    pub super_fast: GasPriceResult,
}

/// Parses a formatted gas price string (in gwei) to an u128 value.
pub fn parse_formatted_gas_to_u128(formatted_unit: &str) -> Result<u128, UnitsError> {
    let pu: ParseUnits = parse_units(formatted_unit, "gwei")?;
    match pu {
        ParseUnits::U256(value) => {
            value.try_into().map_err(|_| UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))
        }
        ParseUnits::I256(value) => {
            if value.is_negative() {
                return Err(UnitsError::ParseSigned(ParseSignedError::IntegerOverflow));
            }
            value.try_into().map_err(|_| UnitsError::ParseSigned(ParseSignedError::IntegerOverflow))
        }
    }
}

#[derive(Error, Debug)]
pub enum GasEstimatorError {
    #[error("Could not get response from provider: {0}")]
    ReqwestError(reqwest::Error),

    #[error("Custom provider error: {0}")]
    CustomError(String),

    #[error("Units error from provider: {0}")]
    UnitsError(UnitsError),

    #[error("Could not work out gas")]
    CanNotWorkOutGasEstimation,

    #[error("Client creation error: {0}")]
    ClientCreationError(#[from] RetryClientError),
}

impl From<reqwest::Error> for GasEstimatorError {
    fn from(error: reqwest::Error) -> Self {
        GasEstimatorError::ReqwestError(error)
    }
}

#[async_trait]
pub trait BaseGasFeeEstimator {
    async fn get_gas_prices(
        &self,
        chain_id: &ChainId,
    ) -> Result<GasEstimatorResult, GasEstimatorError>;
    fn is_chain_supported(&self, chain_id: &ChainId) -> bool;
}

/// Creates and returns the appropriate gas fee estimator based on configuration.
pub async fn get_gas_estimator(
    provider_urls: &[String],
    setup_config: &SetupConfig,
    network: &NetworkSetupConfig,
) -> Result<Arc<dyn BaseGasFeeEstimator + Send + Sync>, GasEstimatorError> {
    if let Some(setup_gas_providers) = &setup_config.gas_providers {
        if let Some(network_gas_provider) = &network.gas_provider {
            match network_gas_provider {
                GasProvider::BLOCKNATIVE => {
                    if let Some(setup) = &setup_gas_providers.blocknative {
                        return Ok(Arc::new(BlockNativeGasFeeEstimator::new(setup.clone())?));
                    }
                }
                GasProvider::ETHERSCAN => {
                    if let Some(setup) = &setup_gas_providers.etherscan {
                        return Ok(Arc::new(EtherscanGasFeeEstimator::new(setup.clone())?));
                    }
                }
                GasProvider::TENDERLY => {
                    if let Some(setup) = &setup_gas_providers.tenderly {
                        return Ok(Arc::new(TenderlyGasFeeEstimator::new(&setup.api_key)));
                    }
                }
                GasProvider::INFURA => {
                    if let Some(setup) = &setup_gas_providers.infura {
                        return Ok(Arc::new(InfuraGasFeeEstimator::new(
                            &setup.api_key,
                            &setup.secret,
                        )));
                    }
                }
                GasProvider::CUSTOM => {
                    if let Some(setup) = &setup_gas_providers.custom {
                        return Ok(Arc::new(setup.to_owned()));
                    }
                }
            }
        }
    }

    let provider = create_retry_client(&provider_urls[0]).await?;
    Ok(Arc::new(FallbackGasFeeEstimator::new(provider.clone())))
}
