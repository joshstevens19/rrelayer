use crate::tests::test_runner::TestRunner;
use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

pub type TestFunction = fn(&TestRunner) -> Pin<Box<dyn Future<Output = Result<()>> + '_>>;

#[derive(Clone)]
pub struct TestDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub function: TestFunction,
}

impl TestDefinition {
    pub fn new(name: &'static str, description: &'static str, function: TestFunction) -> Self {
        Self { name, description, function }
    }
}

pub trait TestModule {
    fn get_tests() -> Vec<TestDefinition>;
}

pub struct TestRegistry;

impl TestRegistry {
    pub fn get_all_tests() -> Vec<TestDefinition> {
        let mut tests = Vec::new();

        // do not move this one as it always needs to use this relayer
        tests.extend(crate::tests::allowlist::AllowlistTests::get_tests());
        tests.extend(crate::tests::authentication::AuthenticationTests::get_tests());
        tests.extend(crate::tests::automatic_top_up::AutomaticTopUpTests::get_tests());
        tests.extend(crate::tests::gas::GasTests::get_tests());
        tests.extend(crate::tests::network::NetworkTests::get_tests());
        tests.extend(crate::tests::rate_limiting::RateLimitingTests::get_tests());
        tests.extend(crate::tests::relayers::RelayerTests::get_tests());
        tests.extend(crate::tests::signing::SigningTests::get_tests());
        tests.extend(crate::tests::webhook::WebhookTests::get_tests());
        tests.extend(crate::tests::transactions::TransactionTests::get_tests());

        tests
    }

    pub fn execute_test<'a>(
        runner: &'a TestRunner,
        test_name: &str,
    ) -> Option<Pin<Box<dyn Future<Output = Result<()>> + 'a>>> {
        let tests = Self::get_all_tests();
        if let Some(test_def) = tests.iter().find(|t| t.name == test_name) {
            Some((test_def.function)(runner))
        } else {
            None
        }
    }
}
