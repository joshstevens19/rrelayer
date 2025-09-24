mod restrictions;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct AllowlistTests;

impl TestModule for AllowlistTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![TestDefinition::new(
            "allowlist_restrictions",
            "Allowlist restriction enforcement",
            |runner| Box::pin(runner.allowlist_restrictions()),
        )]
    }
}
