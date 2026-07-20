mod signing_message_global_limits;
mod signing_message_user_limits;
mod signing_typed_data_global_limits;
mod signing_typed_data_user_limits;
mod transactions_global_limits;
mod transactions_user_limits;

use crate::tests::registry::{TestDefinition, TestModule};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RATE_LIMIT_WINDOW_SECS: u64 = 30;

async fn wait_for_rate_limit_reset() {
    tokio::time::sleep(Duration::from_secs(RATE_LIMIT_WINDOW_SECS + 1)).await;
}

async fn wait_for_rate_limit_window_headroom() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after UNIX_EPOCH")
        .as_secs();
    let seconds_into_window = now % RATE_LIMIT_WINDOW_SECS;

    if seconds_into_window > 10 {
        tokio::time::sleep(Duration::from_secs(RATE_LIMIT_WINDOW_SECS - seconds_into_window + 1))
            .await;
    }
}

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
