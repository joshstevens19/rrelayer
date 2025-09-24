mod clone;
mod concurrent_creation;
mod creation;
mod delete;
mod gas_configuration;
mod pause_unpause;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct RelayerTests;

impl TestModule for RelayerTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![
            TestDefinition::new("relayer_creation", "Basic relayer creation", |runner| {
                Box::pin(runner.relayer_creation())
            }),
            TestDefinition::new(
                "relayer_concurrent_creation",
                "Concurrent relayer creation",
                |runner| Box::pin(runner.relayer_concurrent_creation()),
            ),
            TestDefinition::new("relayer_delete", "Relayer deletion functionality", |runner| {
                Box::pin(runner.relayer_delete())
            }),
            TestDefinition::new("relayer_clone", "Relayer cloning functionality", |runner| {
                Box::pin(runner.relayer_clone())
            }),
            TestDefinition::new(
                "relayer_pause_unpause",
                "Relayer pause/unpause functionality",
                |runner| Box::pin(runner.relayer_pause_unpause()),
            ),
            TestDefinition::new(
                "relayer_gas_configuration",
                "Relayer gas configuration management",
                |runner| Box::pin(runner.relayer_gas_configuration()),
            ),
        ]
    }
}
