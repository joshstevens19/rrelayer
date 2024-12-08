use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use super::ChainId;

#[derive(EnumIter, Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Chain {
    EthereumMainnet,
    EthereumGoerli,
    EthereumSepolia,
    ArbitrumMainnet,
    ArbitrumNova,
    Avalanche,
    Base,
    Binance,
    OpBnbLayer2,
    Cronos,
    Fantom,
    Filecoin,
    LineaMainnet,
    LineaTestnet,
    Optimism,
    PolygonMainnet,
    PolygonMumbai,
    ZkSyncEraMainnet,
}

impl Chain {
    pub fn chain_id(&self) -> ChainId {
        let result: u64 = match self {
            Self::EthereumMainnet => 1,
            Self::EthereumGoerli => 5,
            Self::EthereumSepolia => 11155111,
            Self::ArbitrumMainnet => 42161,
            Self::ArbitrumNova => 42170,
            Self::Avalanche => 43114,
            Self::Base => 8453,
            Self::Binance => 56,
            Self::OpBnbLayer2 => 204,
            Self::Cronos => 25,
            Self::Fantom => 250,
            Self::Filecoin => 314,
            Self::LineaMainnet => 59144,
            Self::LineaTestnet => 59140,
            Self::Optimism => 10,
            Self::PolygonMainnet => 137,
            Self::PolygonMumbai => 100,
            Self::ZkSyncEraMainnet => 324,
        };

        ChainId::new(result)
    }
}
