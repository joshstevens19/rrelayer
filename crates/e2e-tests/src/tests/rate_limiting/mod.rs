mod signing_message_user_limits;
mod signing_typed_data_user_limits;
mod transactions_user_limits;
mod transactions_global_limits;
mod signing_message_global_limits;
mod signing_typed_data_global_limits;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct RateLimitingTests;

impl TestModule for RateLimitingTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![
            TestDefinition::new(
                "rate_limiting_transaction_user_limits",
                "Rate limiting for transactions user limits",
                |runner| Box::pin(runner.rate_limiting_transaction_user_limits()),
            ),
            TestDefinition::new(
                "rate_limiting_transaction_global_limits",
                "Rate limiting for transactions global limits",
                |runner| Box::pin(runner.rate_limiting_transaction_global_limits()),
            ),
            TestDefinition::new(
                "rate_limiting_signing_message_user_limits",
                "Rate limiting for signing messages user limits",
                |runner| Box::pin(runner.rate_limiting_signing_message_user_limits()),
            ),
            TestDefinition::new(
                "rate_limiting_signing_typed_data_user_limits",
                "Rate limiting for signing typed data user limits",
                |runner| Box::pin(runner.rate_limiting_signing_typed_data_user_limits()),
            ),
            TestDefinition::new(
                "rate_limiting_signing_message_global_limits",
                "Rate limiting for signing messages global limits",
                |runner| Box::pin(runner.rate_limiting_signing_message_global_limits()),
            ),
            TestDefinition::new(
                "rate_limiting_signing_typed_data_global_limits",
                "Rate limiting for signing typed data global limits",
                |runner| Box::pin(runner.rate_limiting_signing_typed_data_global_limits()),
            ),
        ]
    }
}
