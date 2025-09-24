use std::time::Duration;

#[derive(Debug, Clone)]
pub enum TestResult {
    Passed,
    Failed(String),
    Timeout,
    Skipped(String),
}

impl TestResult {
    pub fn is_success(&self) -> bool {
        matches!(self, TestResult::Passed)
    }

    pub fn status_icon(&self) -> &'static str {
        match self {
            TestResult::Passed => "PASS",
            TestResult::Failed(_) => "FAIL",
            TestResult::Timeout => "TIMEOUT",
            TestResult::Skipped(_) => "SKIP",
        }
    }
}

#[derive(Debug)]
pub struct TestInfo {
    pub name: String,
    pub result: TestResult,
    pub duration: Duration,
    pub error_message: Option<String>,
}

impl TestInfo {
    pub fn new(name: String, result: TestResult, duration: Duration) -> Self {
        let error_message = match &result {
            TestResult::Failed(msg) => Some(msg.clone()),
            TestResult::Timeout => Some("Test timed out after 180 seconds".to_string()),
            TestResult::Skipped(msg) => Some(msg.clone()),
            TestResult::Passed => None,
        };

        Self { name, result, duration, error_message }
    }
}

pub struct TestSuite {
    pub name: String,
    pub tests: Vec<TestInfo>,
    pub duration: Duration,
}

impl TestSuite {
    pub fn new(name: String) -> Self {
        Self { name, tests: Vec::new(), duration: Duration::ZERO }
    }

    pub fn add_test(&mut self, test: TestInfo) {
        self.duration += test.duration;
        self.tests.push(test);
    }

    pub fn passed_count(&self) -> usize {
        self.tests.iter().filter(|t| t.result.is_success()).count()
    }

    pub fn failed_count(&self) -> usize {
        self.tests.iter().filter(|t| matches!(t.result, TestResult::Failed(_))).count()
    }

    pub fn timeout_count(&self) -> usize {
        self.tests.iter().filter(|t| matches!(t.result, TestResult::Timeout)).count()
    }

    pub fn skipped_count(&self) -> usize {
        self.tests.iter().filter(|t| matches!(t.result, TestResult::Skipped(_))).count()
    }

    pub fn total_count(&self) -> usize {
        self.tests.len()
    }
}
