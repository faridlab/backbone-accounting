//! API Test Base Class
//!
//! Provides HTTP client capabilities for API integration testing.

use reqwest::{header::HeaderMap, Client, Method};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::base_test::{TestError, TestResult};

// ============================================================
// API Response Container
// ============================================================

/// Standardized API response container
#[derive(Debug, Clone)]
pub struct ApiResponse {
    /// Whether the request succeeded (no network errors)
    pub success: bool,

    /// HTTP status code
    pub status_code: u16,

    /// Response headers
    pub headers: HashMap<String, String>,

    /// Raw response body
    pub body: String,

    /// Request duration in milliseconds
    pub duration_ms: u64,
}

impl ApiResponse {
    /// Check if response has success status (2xx)
    pub fn is_success(&self) -> bool {
        self.status_code >= 200 && self.status_code < 300
    }

    /// Parse body as JSON
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.body)
    }

    /// Get body as JSON Value
    pub fn json_value(&self) -> Result<Value, serde_json::Error> {
        serde_json::from_str(&self.body)
    }
}

// ============================================================
// API Test Base
// ============================================================

/// Base struct for API tests with HTTP client capabilities.
pub struct ApiTest {
    /// Test name
    pub name: String,

    /// Base URL for API requests
    pub api_base_url: String,

    /// HTTP client
    pub client: Client,

    /// Default headers for all requests
    pub default_headers: HeaderMap,

    /// Request timeout
    pub timeout: Duration,
}

impl ApiTest {
    /// Create a new API test instance
    pub fn new(name: impl Into<String>, api_base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let mut default_headers = HeaderMap::new();
        default_headers.insert("Content-Type", "application/json".parse().unwrap());
        default_headers.insert("Accept", "application/json".parse().unwrap());

        Self {
            name: name.into(),
            api_base_url: api_base_url.into(),
            client,
            default_headers,
            timeout: Duration::from_secs(30),
        }
    }

    /// Set authorization header (Bearer token)
    pub fn with_auth(mut self, token: &str) -> Self {
        self.default_headers.insert(
            "Authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        self
    }

    /// Set custom timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Add a default header
    pub fn add_header(&mut self, key: &str, value: &str) {
        use reqwest::header::HeaderName;
        let header_name = HeaderName::try_from(key).expect("Invalid header name");
        self.default_headers
            .insert(header_name, value.parse().expect("Invalid header value"));
    }

    /// Build full URL from endpoint
    fn build_url(&self, endpoint: &str) -> String {
        format!("{}{}", self.api_base_url, endpoint)
    }

    /// Convert reqwest headers to HashMap
    fn headers_to_map(headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
        headers
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|val| (k.to_string(), val.to_string()))
            })
            .collect()
    }

    /// Make an HTTP request
    pub async fn request(
        &self,
        method: Method,
        endpoint: &str,
        headers: Option<HeaderMap>,
        body: Option<Value>,
    ) -> Result<ApiResponse, TestError> {
        let url = self.build_url(endpoint);
        let start = Instant::now();

        let mut request = self.client.request(method, &url);

        // Apply default headers
        for (key, value) in self.default_headers.iter() {
            request = request.header(key, value);
        }

        // Apply custom headers
        if let Some(hdrs) = headers {
            for (key, value) in hdrs.iter() {
                request = request.header(key, value);
            }
        }

        // Apply body
        if let Some(b) = body {
            request = request.json(&b);
        }

        let response = request
            .send()
            .await
            .map_err(|e| TestError::HttpError(e.to_string()))?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let status_code = response.status().as_u16();
        let headers = Self::headers_to_map(response.headers());
        let body = response
            .text()
            .await
            .map_err(|e| TestError::HttpError(e.to_string()))?;

        Ok(ApiResponse {
            success: true,
            status_code,
            headers,
            body,
            duration_ms,
        })
    }

    /// Make a GET request
    pub async fn get(
        &self,
        endpoint: &str,
        headers: Option<HeaderMap>,
    ) -> Result<ApiResponse, TestError> {
        self.request(Method::GET, endpoint, headers, None).await
    }

    /// Make a POST request
    pub async fn post<T: Serialize>(
        &self,
        endpoint: &str,
        body: &T,
        headers: Option<HeaderMap>,
    ) -> Result<ApiResponse, TestError> {
        let json = serde_json::to_value(body).map_err(|e| TestError::HttpError(e.to_string()))?;
        self.request(Method::POST, endpoint, headers, Some(json))
            .await
    }

    /// Make a PUT request
    pub async fn put<T: Serialize>(
        &self,
        endpoint: &str,
        body: &T,
        headers: Option<HeaderMap>,
    ) -> Result<ApiResponse, TestError> {
        let json = serde_json::to_value(body).map_err(|e| TestError::HttpError(e.to_string()))?;
        self.request(Method::PUT, endpoint, headers, Some(json))
            .await
    }

    /// Make a PATCH request
    pub async fn patch<T: Serialize>(
        &self,
        endpoint: &str,
        body: &T,
        headers: Option<HeaderMap>,
    ) -> Result<ApiResponse, TestError> {
        let json = serde_json::to_value(body).map_err(|e| TestError::HttpError(e.to_string()))?;
        self.request(Method::PATCH, endpoint, headers, Some(json))
            .await
    }

    /// Make a DELETE request
    pub async fn delete(
        &self,
        endpoint: &str,
        headers: Option<HeaderMap>,
    ) -> Result<ApiResponse, TestError> {
        self.request(Method::DELETE, endpoint, headers, None).await
    }

    /// Validate response status code
    pub fn validate_status(
        &self,
        response: &ApiResponse,
        expected_status: u16,
    ) -> Result<(), String> {
        if response.status_code != expected_status {
            return Err(format!(
                "Expected status {}, got {}",
                expected_status, response.status_code
            ));
        }
        Ok(())
    }

    /// Validate response and return parsed body
    pub fn validate_and_parse<T: DeserializeOwned>(
        &self,
        response: &ApiResponse,
        expected_status: u16,
    ) -> Result<T, String> {
        self.validate_status(response, expected_status)?;
        response.json().map_err(|e| format!("JSON parse error: {}", e))
    }

    /// Create a test result from response validation
    pub fn create_result(
        &self,
        test_name: &str,
        response: &ApiResponse,
        expected_status: u16,
        success_message: &str,
    ) -> TestResult {
        let input = serde_json::json!({
            "expected_status": expected_status,
        });

        let output = serde_json::json!({
            "status_code": response.status_code,
            "body": response.body,
        });

        match self.validate_status(response, expected_status) {
            Ok(_) => TestResult::success(test_name, success_message)
                .with_duration(response.duration_ms as f64 / 1000.0)
                .with_input(input)
                .with_output(output),
            Err(e) => TestResult::failure(test_name, e)
                .with_duration(response.duration_ms as f64 / 1000.0)
                .with_input(input)
                .with_output(output),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_is_success() {
        let response = ApiResponse {
            success: true,
            status_code: 200,
            headers: HashMap::new(),
            body: "{}".to_string(),
            duration_ms: 100,
        };
        assert!(response.is_success());

        let response_404 = ApiResponse {
            success: true,
            status_code: 404,
            headers: HashMap::new(),
            body: "{}".to_string(),
            duration_ms: 100,
        };
        assert!(!response_404.is_success());
    }

    #[test]
    fn test_build_url() {
        let api_test = ApiTest::new("test", "http://localhost:3000");
        assert_eq!(
            api_test.build_url("/api/v1/users"),
            "http://localhost:3000/api/v1/users"
        );
    }
}
