//! Integration Test Framework for Accounting Module
//!
//! This module implements a layered integration test framework.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │         Test Runner / CLI               │
//! ├─────────────────────────────────────────┤
//! │         Specific Test Classes           │
//! │         (tests/*.rs)                    │
//! ├─────────────────────────────────────────┤
//! │         Base Test Classes               │
//! │         (framework/base_test.rs)        │
//! ├─────────────────────────────────────────┤
//! │         Common Utilities                │
//! │         (helpers/*.rs)                  │
//! └─────────────────────────────────────────┘
//! ```

pub mod framework;
pub mod helpers;
pub mod tests;

// Re-export commonly used types
pub use framework::{Test, TestResult, TestSuiteResult, TestError, ApiTest, ApiResponse};
pub use helpers::{CommonUtils, JwtTokenManager};
