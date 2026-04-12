//! Core Framework Components
//!
//! Provides base traits, result containers, and API test utilities.

mod base_test;
mod api_test;

pub use base_test::{Test, TestResult, TestSuiteResult, TestError};
pub use api_test::{ApiTest, ApiResponse};
