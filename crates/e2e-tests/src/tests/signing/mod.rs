mod text;
mod typed_data;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct SigningTests;

impl TestModule for SigningTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![
            TestDefinition::new("signing_text", "Text signing functionality", |runner| {
                Box::pin(runner.signing_text())
            }),
            TestDefinition::new(
                "signing_typed_data",
                "Typed data signing functionality",
                |runner| Box::pin(runner.signing_typed_data()),
            ),
        ]
    }
}
