use super::Relayer;
use crate::provider::EvmProvider;

pub struct RelayerProviderContext<'a> {
    pub relayer: Relayer,
    pub provider: &'a EvmProvider,
}
