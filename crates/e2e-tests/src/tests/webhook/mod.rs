mod delivery;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct WebhookTests;

impl TestModule for WebhookTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![TestDefinition::new("webhook_delivery", "Webhook delivery testing", |runner| {
            Box::pin(runner.webhook_delivery())
        })]
    }
}
