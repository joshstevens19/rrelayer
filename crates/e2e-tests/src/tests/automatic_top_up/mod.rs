mod erc20;
mod native;
mod safe_proxy;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct AutomaticTopUpTests;

impl TestModule for AutomaticTopUpTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![
            TestDefinition::new(
                "automatic_top_up_native",
                "Automatic top-up with native tokens",
                |runner| Box::pin(runner.automatic_top_up_native()),
            ),
            TestDefinition::new(
                "automatic_top_up_erc20",
                "Automatic top-up with ERC20 tokens",
                |runner| Box::pin(runner.automatic_top_up_erc20()),
            ),
            TestDefinition::new(
                "automatic_top_up_safe_proxy",
                "Automatic top-up with Safe proxy",
                |runner| Box::pin(runner.automatic_top_up_safe_proxy()),
            ),
        ]
    }
}
