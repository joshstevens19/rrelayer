mod add;
mod edge_cases;
mod remove;
mod restrictions;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct AllowlistTests;

impl TestModule for AllowlistTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![
            TestDefinition::new("allowlist_add", "Allowlist add operation", |runner| {
                Box::pin(runner.allowlist_add())
            }),
            TestDefinition::new("allowlist_remove", "Allowlist remove operation", |runner| {
                Box::pin(runner.allowlist_remove())
            }),
            TestDefinition::new(
                "allowlist_restrictions",
                "Allowlist restriction enforcement",
                |runner| Box::pin(runner.allowlist_restrictions()),
            ),
            TestDefinition::new("allowlist_edge_cases", "Allowlist edge case handling", |runner| {
                Box::pin(runner.allowlist_edge_cases())
            }),
        ]
    }
}
