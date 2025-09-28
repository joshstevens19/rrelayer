mod all;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct NetworkTests;

impl TestModule for NetworkTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![TestDefinition::new("all_networks", "All networks", |runner| {
            Box::pin(runner.all_networks())
        })]
    }
}
