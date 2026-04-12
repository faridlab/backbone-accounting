//! Common Test Utilities
//!
//! Aggregates utilities and provides high-level helper methods for testing.

use serde_json::Value;
use std::collections::HashMap;

use super::jwt_manager::JwtTokenManager;
use crate::integration::framework::ApiResponse;

/// Common utilities for integration tests
pub struct CommonUtils {
    /// JWT token manager
    pub jwt_manager: JwtTokenManager,

    /// Generated test data
    test_data: HashMap<String, Value>,
}

impl CommonUtils {
    /// Create new common utils with JWT manager
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            jwt_manager: JwtTokenManager::new(jwt_secret),
            test_data: HashMap::new(),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new("test-secret-key-for-integration-tests")
    }

    /// Simulate an authenticated session
    ///
    /// Creates a JWT token and returns (user_id, token)
    pub fn simulate_auth_session(
        &self,
        user_prefix: &str,
    ) -> Result<(String, String), String> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let user_id = format!("{}-u-{}", user_prefix, timestamp);

        let (token, _) = self.jwt_manager.create_token(&user_id, None)?;

        Ok((user_id, token))
    }

    /// Simulate an admin session
    pub fn simulate_admin_session(
        &self,
        user_prefix: &str,
    ) -> Result<(String, String), String> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let user_id = format!("{}-admin-{}", user_prefix, timestamp);

        let (token, _) = self.jwt_manager.create_admin_token(&user_id)?;

        Ok((user_id, token))
    }

    // ============================================================
    // Response Validation
    // ============================================================

    /// Validate API response status and return parsed body
    pub fn validate_api_response(
        &self,
        response: &ApiResponse,
        expected_status: u16,
    ) -> Result<(bool, String, Option<Value>), String> {
        if response.status_code != expected_status {
            return Ok((
                false,
                format!(
                    "Status mismatch: expected {}, got {}",
                    expected_status, response.status_code
                ),
                None,
            ));
        }

        let body = serde_json::from_str(&response.body).ok();
        Ok((true, "Success".to_string(), body))
    }

    /// Validate response with expected error code
    pub fn validate_error_response(
        &self,
        response: &ApiResponse,
        expected_status: u16,
        expected_error_code: Option<&str>,
    ) -> Result<(bool, String), String> {
        if response.status_code != expected_status {
            return Ok((
                false,
                format!(
                    "Status mismatch: expected {}, got {}",
                    expected_status, response.status_code
                ),
            ));
        }

        if let Some(code) = expected_error_code {
            let body: Value = serde_json::from_str(&response.body)
                .map_err(|e| format!("Failed to parse error body: {}", e))?;

            let actual_code = body
                .get("error_code")
                .or_else(|| body.get("code"))
                .and_then(|v| v.as_str());

            if actual_code != Some(code) {
                return Ok((
                    false,
                    format!(
                        "Error code mismatch: expected {}, got {:?}",
                        code, actual_code
                    ),
                ));
            }
        }

        Ok((true, "Error response validated".to_string()))
    }

    // ============================================================
    // Object Comparison
    // ============================================================

    /// Compare nested objects and return list of mismatches
    pub fn compare_nested_objects(
        &self,
        expected: &Value,
        actual: &Value,
        path: &str,
    ) -> Vec<String> {
        let mut mismatches = Vec::new();

        match (expected, actual) {
            (Value::Object(exp_obj), Value::Object(act_obj)) => {
                for (key, exp_value) in exp_obj {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };

                    if let Some(act_value) = act_obj.get(key) {
                        mismatches.extend(self.compare_nested_objects(exp_value, act_value, &new_path));
                    } else {
                        mismatches.push(format!("{}: expected key missing in actual", new_path));
                    }
                }
            }
            (Value::Array(exp_arr), Value::Array(act_arr)) => {
                if exp_arr.len() != act_arr.len() {
                    mismatches.push(format!(
                        "{}: array length mismatch (expected {}, got {})",
                        path,
                        exp_arr.len(),
                        act_arr.len()
                    ));
                }
                for (i, (exp, act)) in exp_arr.iter().zip(act_arr.iter()).enumerate() {
                    let new_path = format!("{}[{}]", path, i);
                    mismatches.extend(self.compare_nested_objects(exp, act, &new_path));
                }
            }
            _ => {
                if expected != actual {
                    mismatches.push(format!(
                        "{}: expected {:?}, got {:?}",
                        path, expected, actual
                    ));
                }
            }
        }

        mismatches
    }

    /// Check if a value contains expected fields
    pub fn contains_fields(&self, value: &Value, fields: &[&str]) -> bool {
        if let Value::Object(obj) = value {
            fields.iter().all(|field| obj.contains_key(*field))
        } else {
            false
        }
    }

    // ============================================================
    // Test Data Generation
    // ============================================================

    /// Generate a unique ID with prefix
    pub fn generate_id(&self, prefix: &str) -> String {
        let unique = uuid::Uuid::new_v4().to_string();
        format!("{}-{}", prefix, &unique[..8])
    }

    /// Generate a unique email
    pub fn generate_email(&self, prefix: &str) -> String {
        format!("{}@test.local", self.generate_id(prefix))
    }

    /// Generate a valid test password
    pub fn generate_password(&self) -> String {
        "TestPassword123!".to_string()
    }

    /// Store test data for later retrieval
    pub fn store_data(&mut self, key: &str, value: Value) {
        self.test_data.insert(key.to_string(), value);
    }

    /// Retrieve stored test data
    pub fn get_data(&self, key: &str) -> Option<&Value> {
        self.test_data.get(key)
    }

    /// Clear stored test data
    pub fn clear_data(&mut self) {
        self.test_data.clear();
    }
}

// ============================================================
// Validation Result
// ============================================================

/// Result of a validation operation
#[derive(Debug)]
pub struct ValidationResult {
    pub success: bool,
    pub message: String,
    pub details: Vec<String>,
}

impl ValidationResult {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            details: Vec::new(),
        }
    }

    pub fn fail(message: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulate_auth_session() {
        let utils = CommonUtils::default();
        let result = utils.simulate_auth_session("test");
        assert!(result.is_ok());

        let (user_id, token) = result.unwrap();
        assert!(user_id.starts_with("test-u-"));
        assert!(!token.is_empty());
    }

    #[test]
    fn test_compare_nested_objects() {
        let utils = CommonUtils::default();

        let expected = serde_json::json!({
            "name": "John",
            "age": 30
        });

        let actual = serde_json::json!({
            "name": "John",
            "age": 31
        });

        let mismatches = utils.compare_nested_objects(&expected, &actual, "");
        assert_eq!(mismatches.len(), 1);
        assert!(mismatches[0].contains("age"));
    }

    #[test]
    fn test_contains_fields() {
        let utils = CommonUtils::default();

        let value = serde_json::json!({
            "id": "123",
            "name": "Test",
            "email": "test@example.com"
        });

        assert!(utils.contains_fields(&value, &["id", "name"]));
        assert!(!utils.contains_fields(&value, &["id", "missing_field"]));
    }
}
