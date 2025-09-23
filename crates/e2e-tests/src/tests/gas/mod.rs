mod price;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct GasTests;

impl TestModule for GasTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![TestDefinition::new("gas_price_api", "Gas price API functionality", |runner| {
            Box::pin(runner.gas_price())
        })]
    }
}
