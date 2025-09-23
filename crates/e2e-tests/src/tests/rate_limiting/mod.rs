mod signing_message;
mod signing_typed_data;
mod transactions;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct RateLimitingTests;

impl TestModule for RateLimitingTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![
            TestDefinition::new(
                "rate_limiting_transaction",
                "Rate limiting for transactions",
                |runner| Box::pin(runner.rate_limiting_transaction()),
            ),
            TestDefinition::new(
                "rate_limiting_signing_message",
                "Rate limiting for signing messages",
                |runner| Box::pin(runner.rate_limiting_signing_message()),
            ),
            TestDefinition::new(
                "rate_limiting_signing_typed_data",
                "Rate limiting for signing typed data",
                |runner| Box::pin(runner.rate_limiting_signing_typed_data()),
            ),
        ]
    }
}
