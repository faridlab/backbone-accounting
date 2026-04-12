//! Test Setup Manager
//!
//! Manages common setup tasks for integration tests.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crate::integration::framework::TestError;
use super::JwtTokenManager;

/// Configuration for test setup
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Base URL for API
    pub api_base_url: String,

    /// JWT secret key
    pub jwt_secret: String,

    /// Results directory for test outputs
    pub results_dir: PathBuf,

    /// Whether to enable verbose logging
    pub verbose: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            api_base_url: env::var("API_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:3000".to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "test-secret-key".to_string()),
            results_dir: PathBuf::from(
                env::var("TEST_RESULTS_DIR").unwrap_or_else(|_| "./test-results".to_string()),
            ),
            verbose: env::var("TEST_VERBOSE")
                .map(|v| v == "1" || v.to_lowercase() == "true")
                .unwrap_or(false),
        }
    }
}

/// Resources created during setup that need cleanup
#[derive(Debug, Default)]
pub struct TestResources {
    /// Entity IDs created during tests
    pub entity_ids: Vec<String>,

    /// Cache keys to delete
    pub cache_keys: Vec<String>,

    /// Custom resource tracking
    pub custom: HashMap<String, Vec<String>>,
}

impl TestResources {
    pub fn new() -> Self {
        Self::default()
    }

    /// Track an entity ID for cleanup
    pub fn track_entity(&mut self, id: impl Into<String>) {
        self.entity_ids.push(id.into());
    }

    /// Track a cache key for cleanup
    pub fn track_cache_key(&mut self, key: impl Into<String>) {
        self.cache_keys.push(key.into());
    }

    /// Track a custom resource type
    pub fn track_custom(&mut self, resource_type: &str, id: impl Into<String>) {
        self.custom
            .entry(resource_type.to_string())
            .or_default()
            .push(id.into());
    }

    /// Clear all tracked resources
    pub fn clear(&mut self) {
        self.entity_ids.clear();
        self.cache_keys.clear();
        self.custom.clear();
    }
}

/// Manages test setup and provides common resources
pub struct TestSetupManager {
    /// Test configuration
    pub config: TestConfig,

    /// JWT token manager
    pub jwt_manager: JwtTokenManager,

    /// Tracked resources for cleanup
    pub resources: TestResources,
}

impl TestSetupManager {
    /// Create a new test setup manager with default configuration
    pub fn new() -> Self {
        let config = TestConfig::default();
        let jwt_manager = JwtTokenManager::new(&config.jwt_secret);

        Self {
            config,
            jwt_manager,
            resources: TestResources::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: TestConfig) -> Self {
        let jwt_manager = JwtTokenManager::new(&config.jwt_secret);

        Self {
            config,
            jwt_manager,
            resources: TestResources::new(),
        }
    }

    /// Setup common test infrastructure
    pub async fn setup(&mut self) -> Result<(), TestError> {
        // Create results directory if needed
        if !self.config.results_dir.exists() {
            std::fs::create_dir_all(&self.config.results_dir)
                .map_err(|e| TestError::SetupFailed(format!("Failed to create results dir: {}", e)))?;
        }

        if self.config.verbose {
            println!("[Setup] API Base URL: {}", self.config.api_base_url);
            println!("[Setup] Results Dir: {:?}", self.config.results_dir);
        }

        Ok(())
    }

    /// Teardown and cleanup all tracked resources
    pub async fn teardown(&mut self) -> Result<(), TestError> {
        if self.config.verbose {
            println!(
                "[Teardown] Cleaning up: {} entities",
                self.resources.entity_ids.len()
            );
        }

        // Clear tracked resources
        self.resources.clear();

        Ok(())
    }

    /// Generate a unique test ID with prefix
    pub fn generate_test_id(&self, prefix: &str) -> String {
        let unique = uuid::Uuid::new_v4().to_string();
        format!("{}_{}", prefix, &unique[..8])
    }

    /// Generate a unique test email
    pub fn generate_test_email(&self, prefix: &str) -> String {
        let id = self.generate_test_id(prefix);
        format!("{}@test.local", id)
    }

    /// Generate a unique test name
    pub fn generate_test_name(&self, prefix: &str) -> String {
        self.generate_test_id(prefix)
    }

    /// Create an auth token for testing
    pub fn create_auth_token(&self, user_id: &str) -> Result<String, TestError> {
        let (token, _) = self
            .jwt_manager
            .create_token(user_id, None)
            .map_err(|e| TestError::SetupFailed(e))?;
        Ok(token)
    }

    /// Create an admin auth token for testing
    pub fn create_admin_token(&self, user_id: &str) -> Result<String, TestError> {
        let (token, _) = self
            .jwt_manager
            .create_admin_token(user_id)
            .map_err(|e| TestError::SetupFailed(e))?;
        Ok(token)
    }

    /// Save test results to file
    pub fn save_results<T: serde::Serialize>(
        &self,
        filename: &str,
        data: &T,
    ) -> Result<(), TestError> {
        let path = self.config.results_dir.join(filename);
        let json = serde_json::to_string_pretty(data)
            .map_err(|e| TestError::ConfigError(format!("Failed to serialize results: {}", e)))?;
        std::fs::write(&path, json)
            .map_err(|e| TestError::ConfigError(format!("Failed to write results: {}", e)))?;
        Ok(())
    }
}

impl Default for TestSetupManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_test_id() {
        let manager = TestSetupManager::new();
        let id1 = manager.generate_test_id("test");
        let id2 = manager.generate_test_id("test");

        assert!(id1.starts_with("test_"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_test_email() {
        let manager = TestSetupManager::new();
        let email = manager.generate_test_email("user");

        assert!(email.ends_with("@test.local"));
        assert!(email.starts_with("user_"));
    }

    #[test]
    fn test_resource_tracking() {
        let mut resources = TestResources::new();

        resources.track_entity("entity-1");
        resources.track_cache_key("cache-1");

        assert_eq!(resources.entity_ids.len(), 1);
        assert_eq!(resources.cache_keys.len(), 1);

        resources.clear();
        assert!(resources.entity_ids.is_empty());
    }
}
