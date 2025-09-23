mod enable_disable;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct NetworkTests;

impl TestModule for NetworkTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![TestDefinition::new(
            "network_disable_enable",
            "Network enable/disable operations",
            |runner| Box::pin(runner.network_disable_enable()),
        )]
    }
}
