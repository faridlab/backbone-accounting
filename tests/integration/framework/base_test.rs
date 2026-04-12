//! Base Test Framework
//!
//! Defines the core test traits and result containers for integration testing.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

// ============================================================
// Error Types
// ============================================================

/// Errors that can occur during test execution
#[derive(Debug, Error)]
pub enum TestError {
    #[error("Setup failed: {0}")]
    SetupFailed(String),

    #[error("Teardown failed: {0}")]
    TeardownFailed(String),

    #[error("Test execution failed: {0}")]
    ExecutionFailed(String),

    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

// ============================================================
// Test Result Container
// ============================================================

/// Standardized container for individual test results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Name of the test
    pub test_name: String,

    /// Pass/fail status
    pub success: bool,

    /// Human-readable description of the result
    pub details: String,

    /// Execution time in seconds
    pub duration_seconds: f64,

    /// Test input data (for debugging)
    pub input: serde_json::Value,

    /// Test output/response (for debugging)
    pub output: serde_json::Value,

    /// When the test was executed
    pub timestamp: DateTime<Utc>,
}

impl TestResult {
    /// Create a new successful test result
    pub fn success(test_name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            test_name: test_name.into(),
            success: true,
            details: details.into(),
            duration_seconds: 0.0,
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            timestamp: Utc::now(),
        }
    }

    /// Create a new failed test result
    pub fn failure(test_name: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            test_name: test_name.into(),
            success: false,
            details: details.into(),
            duration_seconds: 0.0,
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            timestamp: Utc::now(),
        }
    }

    /// Set the duration
    pub fn with_duration(mut self, seconds: f64) -> Self {
        self.duration_seconds = seconds;
        self
    }

    /// Set the input data
    pub fn with_input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    /// Set the output data
    pub fn with_output(mut self, output: serde_json::Value) -> Self {
        self.output = output;
        self
    }
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "✓" } else { "✗" };
        write!(
            f,
            "{} {} ({:.3}s): {}",
            status, self.test_name, self.duration_seconds, self.details
        )
    }
}

// ============================================================
// Test Suite Result Container
// ============================================================

/// Aggregates results from multiple tests in a suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteResult {
    /// Name of the test suite
    pub suite_name: String,

    /// When the suite started
    pub start_time: DateTime<Utc>,

    /// When the suite ended
    pub end_time: Option<DateTime<Utc>>,

    /// Individual test results
    pub results: Vec<TestResult>,
}

impl TestSuiteResult {
    /// Create a new test suite
    pub fn new(suite_name: impl Into<String>) -> Self {
        Self {
            suite_name: suite_name.into(),
            start_time: Utc::now(),
            end_time: None,
            results: Vec::new(),
        }
    }

    /// Add a test result to the suite
    pub fn add_result(&mut self, result: TestResult) {
        self.results.push(result);
    }

    /// Mark the suite as complete
    pub fn complete(&mut self) {
        self.end_time = Some(Utc::now());
    }

    /// Count of passed tests
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.success).count()
    }

    /// Count of failed tests
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| !r.success).count()
    }

    /// Total number of tests
    pub fn total_count(&self) -> usize {
        self.results.len()
    }

    /// Success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.results.is_empty() {
            return 0.0;
        }
        (self.passed_count() as f64 / self.results.len() as f64) * 100.0
    }

    /// Total duration of all tests
    pub fn total_duration(&self) -> f64 {
        self.results.iter().map(|r| r.duration_seconds).sum()
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed_count() == 0 && !self.results.is_empty()
    }

    /// Get failed test results
    pub fn failed_tests(&self) -> Vec<&TestResult> {
        self.results.iter().filter(|r| !r.success).collect()
    }

    /// Save test results to a JSON file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }
}

impl fmt::Display for TestSuiteResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n══════════════════════════════════════════════════════════")?;
        writeln!(f, "Test Suite: {}", self.suite_name)?;
        writeln!(f, "══════════════════════════════════════════════════════════")?;

        for result in &self.results {
            writeln!(f, "  {}", result)?;
        }

        writeln!(f, "──────────────────────────────────────────────────────────")?;
        writeln!(
            f,
            "Results: {} passed, {} failed, {} total ({:.1}%)",
            self.passed_count(),
            self.failed_count(),
            self.total_count(),
            self.success_rate()
        )?;
        writeln!(f, "Duration: {:.3}s", self.total_duration())?;
        writeln!(f, "══════════════════════════════════════════════════════════")?;

        Ok(())
    }
}

// ============================================================
// Base Test Trait
// ============================================================

/// Base trait that all integration tests must implement.
#[async_trait]
pub trait Test: Send + Sync {
    /// Get the name of this test suite
    fn name(&self) -> &str;

    /// Initialize resources
    async fn setup(&mut self) -> Result<(), TestError>;

    /// Cleanup resources
    async fn teardown(&mut self) -> Result<(), TestError>;

    /// Execute all test cases and return results
    async fn run_tests(&mut self) -> Vec<TestResult>;

    /// Main execution flow: setup → run_tests → teardown
    async fn execute(&mut self) -> TestSuiteResult {
        let mut suite = TestSuiteResult::new(self.name());

        // Setup phase
        if let Err(e) = self.setup().await {
            suite.add_result(TestResult::failure(
                format!("{} - Setup", self.name()),
                format!("Setup failed: {}", e),
            ));
            suite.complete();
            return suite;
        }

        // Run tests phase
        let results = self.run_tests().await;
        for result in results {
            suite.add_result(result);
        }

        // Teardown phase
        if let Err(e) = self.teardown().await {
            suite.add_result(TestResult::failure(
                format!("{} - Teardown", self.name()),
                format!("Teardown failed: {}", e),
            ));
        }

        suite.complete();
        suite
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_success() {
        let result = TestResult::success("Test 1", "Passed successfully");
        assert!(result.success);
        assert_eq!(result.test_name, "Test 1");
    }

    #[test]
    fn test_result_failure() {
        let result = TestResult::failure("Test 2", "Failed with error");
        assert!(!result.success);
        assert_eq!(result.details, "Failed with error");
    }

    #[test]
    fn test_suite_statistics() {
        let mut suite = TestSuiteResult::new("Test Suite");
        suite.add_result(TestResult::success("Test 1", "OK"));
        suite.add_result(TestResult::success("Test 2", "OK"));
        suite.add_result(TestResult::failure("Test 3", "Failed"));

        assert_eq!(suite.passed_count(), 2);
        assert_eq!(suite.failed_count(), 1);
        assert_eq!(suite.total_count(), 3);
    }
}
