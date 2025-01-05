use std::sync::Arc;

use alloy::primitives::{
    utils::{parse_units, ParseUnits, UnitsError},
    ParseSignedError,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::infura::InfuraGasFeeEstimator;
use crate::{
    gas::types::{GasPrice, GasProvider, MaxFee, MaxPriorityFee},
    network::types::ChainId,
    setup::yaml::{NetworkSetupConfig, SetupConfig},
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
    // Effective Gas Price = Base Fee + Priority Fee
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

pub fn get_gas_estimator(
    setup_config: &SetupConfig,
    network: &NetworkSetupConfig,
) -> Option<Arc<dyn BaseGasFeeEstimator + Send + Sync>> {
    match &setup_config.gas_providers {
        Some(setup_gas_providers) => {
            if let Some(network_gas_provider) = &network.gas_provider {
                return match network_gas_provider {
                    GasProvider::Infura => match &setup_gas_providers.infura {
                        Some(setup) => Some(Arc::new(InfuraGasFeeEstimator::new(
                            &setup.api_key,
                            &setup.secret,
                        ))),
                        None => None,
                    },
                    GasProvider::Custom => match &setup_gas_providers.custom {
                        Some(setup) => Some(Arc::new(setup.to_owned())),
                        None => None,
                    },
                }
            }
        }
        None => return None,
    }

    None
}
