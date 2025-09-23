mod unauthenticated;

use crate::tests::registry::{TestDefinition, TestModule};

pub struct AuthenticationTests;

impl TestModule for AuthenticationTests {
    fn get_tests() -> Vec<TestDefinition> {
        vec![TestDefinition::new("unauthenticated", "Unauthenticated access testing", |runner| {
            Box::pin(runner.unauthenticated())
        })]
    }
}
