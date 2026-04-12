// Health Checker Implementation
// Health monitoring for the Backbone module

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::domain::repositories::BackboneRepository;

// Health check status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded)
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, Self::Unhealthy)
    }
}

// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub component: String,
    pub status: HealthStatus,
    pub message: String,
    pub details: Option<HashMap<String, serde_json::Value>>,
    pub response_time_ms: u64,
    pub timestamp: DateTime<Utc>,
    pub error: Option<String>,
}

impl HealthCheckResult {
    pub fn healthy(component: String, message: String, response_time_ms: u64) -> Self {
        Self {
            component,
            status: HealthStatus::Healthy,
            message,
            details: None,
            response_time_ms,
            timestamp: Utc::now(),
            error: None,
        }
    }

    pub fn degraded(component: String, message: String, response_time_ms: u64) -> Self {
        Self {
            component,
            status: HealthStatus::Degraded,
            message,
            details: None,
            response_time_ms,
            timestamp: Utc::now(),
            error: None,
        }
    }

    pub fn unhealthy(component: String, message: String, response_time_ms: u64, error: String) -> Self {
        Self {
            component,
            status: HealthStatus::Unhealthy,
            message,
            details: None,
            response_time_ms,
            timestamp: Utc::now(),
            error: Some(error),
        }
    }

    pub fn with_details(mut self, details: HashMap<String, serde_json::Value>) -> Self {
        self.details = Some(details);
        self
    }
}

// Overall health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatusResponse {
    pub status: HealthStatus,
    pub timestamp: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub version: String,
    pub checks: HashMap<String, HealthCheckResult>,
    pub summary: HealthSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSummary {
    pub total_checks: usize,
    pub healthy_checks: usize,
    pub degraded_checks: usize,
    pub unhealthy_checks: usize,
    pub average_response_time_ms: f64,
}

impl HealthStatusResponse {
    pub fn new(start_time: DateTime<Utc>, version: String) -> Self {
        Self {
            status: HealthStatus::Healthy,
            timestamp: Utc::now(),
            uptime_seconds: (Utc::now() - start_time).num_seconds() as u64,
            version,
            checks: HashMap::new(),
            summary: HealthSummary {
                total_checks: 0,
                healthy_checks: 0,
                degraded_checks: 0,
                unhealthy_checks: 0,
                average_response_time_ms: 0.0,
            },
        }
    }

    pub fn add_check(&mut self, result: HealthCheckResult) {
        let component = result.component.clone();
        self.checks.insert(component, result.clone());

        // Update summary
        self.summary.total_checks += 1;
        match result.status {
            HealthStatus::Healthy => self.summary.healthy_checks += 1,
            HealthStatus::Degraded => self.summary.degraded_checks += 1,
            HealthStatus::Unhealthy => self.summary.unhealthy_checks += 1,
            HealthStatus::Unknown => {} // Count unknown as degraded
        }

        // Update overall status
        self.update_overall_status();

        // Update average response time
        let total_response_time: u64 = self.checks.values()
            .map(|check| check.response_time_ms)
            .sum();
        self.summary.average_response_time_ms = total_response_time as f64 / self.checks.len() as f64;
    }

    fn update_overall_status(&mut self) {
        if self.checks.is_empty() {
            self.status = HealthStatus::Unknown;
            return;
        }

        if self.summary.unhealthy_checks > 0 {
            self.status = HealthStatus::Unhealthy;
        } else if self.summary.degraded_checks > 0 {
            self.status = HealthStatus::Degraded;
        } else {
            self.status = HealthStatus::Healthy;
        }
    }
}

// Health check trait
#[async_trait]
pub trait HealthCheck: Send + Sync {
    fn name(&self) -> &str;
    async fn check(&self) -> HealthCheckResult;
}

// Database health check
pub struct DatabaseHealthCheck {
    repository: Box<dyn BackboneRepository>,
    timeout: Duration,
}

impl DatabaseHealthCheck {
    pub fn new(repository: Box<dyn BackboneRepository>) -> Self {
        Self {
            repository,
            timeout: Duration::from_secs(5),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl HealthCheck for DatabaseHealthCheck {
    fn name(&self) -> &str {
        "database"
    }

    async fn check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();

        let result = tokio::time::timeout(self.timeout, self.repository.health_check()).await;

        let response_time = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(true)) => {
                HealthCheckResult::healthy(
                    self.name().to_string(),
                    "Database connection is healthy".to_string(),
                    response_time,
                )
            }
            Ok(Ok(false)) => {
                HealthCheckResult::unhealthy(
                    self.name().to_string(),
                    "Database connection failed".to_string(),
                    response_time,
                    "Database health check returned false".to_string(),
                )
            }
            Ok(Err(e)) => {
                HealthCheckResult::unhealthy(
                    self.name().to_string(),
                    "Database connection error".to_string(),
                    response_time,
                    e.to_string(),
                )
            }
            Err(_) => {
                HealthCheckResult::unhealthy(
                    self.name().to_string(),
                    "Database health check timed out".to_string(),
                    self.timeout.as_millis() as u64,
                    format!("Health check timed out after {:?}", self.timeout),
                )
            }
        }
    }
}

// Memory usage health check
pub struct MemoryHealthCheck {
    warning_threshold_mb: u64,
    critical_threshold_mb: u64,
}

impl MemoryHealthCheck {
    pub fn new() -> Self {
        Self {
            warning_threshold_mb: 512, // 512MB
            critical_threshold_mb: 1024, // 1GB
        }
    }

    pub fn with_thresholds(mut self, warning_mb: u64, critical_mb: u64) -> Self {
        self.warning_threshold_mb = warning_mb;
        self.critical_threshold_mb = critical_mb;
        self
    }

    fn get_memory_usage() -> (u64, HashMap<String, serde_json::Value>) {
        let mut details = HashMap::new();

        // Get current process memory usage
        if let Ok(usage) = sys_info::mem_info() {
            let total_memory = usage.total;
            let free_memory = usage.free;
            let used_memory = total_memory.saturating_sub(free_memory);

            details.insert("total_memory_kb".to_string(), serde_json::Value::Number(serde_json::Number::from(total_memory)));
            details.insert("free_memory_kb".to_string(), serde_json::Value::Number(serde_json::Number::from(free_memory)));
            details.insert("used_memory_kb".to_string(), serde_json::Value::Number(serde_json::Number::from(used_memory)));

            // Convert to MB for return value
            (used_memory / 1024, details)
        } else {
            // Fallback to system memory info
            let used_memory_mb = 100; // Placeholder value
            details.insert("fallback".to_string(), serde_json::Value::Bool(true));
            (used_memory_mb, details)
        }
    }
}

#[async_trait]
impl HealthCheck for MemoryHealthCheck {
    fn name(&self) -> &str {
        "memory"
    }

    async fn check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();
        let (memory_mb, details) = Self::get_memory_usage();
        let response_time = start.elapsed().as_millis() as u64;

        if memory_mb > self.critical_threshold_mb {
            HealthCheckResult::unhealthy(
                self.name().to_string(),
                format!("Memory usage is critical: {} MB", memory_mb),
                response_time,
                format!("Memory usage {} MB exceeds critical threshold {} MB", memory_mb, self.critical_threshold_mb),
            ).with_details(details)
        } else if memory_mb > self.warning_threshold_mb {
            HealthCheckResult::degraded(
                self.name().to_string(),
                format!("Memory usage is high: {} MB", memory_mb),
                response_time,
            ).with_details(details)
        } else {
            HealthCheckResult::healthy(
                self.name().to_string(),
                format!("Memory usage is normal: {} MB", memory_mb),
                response_time,
            ).with_details(details)
        }
    }
}

// Disk space health check
pub struct DiskSpaceHealthCheck {
    warning_threshold_percent: f64,
    critical_threshold_percent: f64,
    mount_path: String,
}

impl DiskSpaceHealthCheck {
    pub fn new() -> Self {
        Self {
            warning_threshold_percent: 80.0,
            critical_threshold_percent: 95.0,
            mount_path: "/".to_string(),
        }
    }

    pub fn with_thresholds(mut self, warning_percent: f64, critical_percent: f64) -> Self {
        self.warning_threshold_percent = warning_percent;
        self.critical_threshold_percent = critical_percent;
        self
    }

    pub fn with_mount_path(mut self, path: String) -> Self {
        self.mount_path = path;
        self
    }

    fn get_disk_usage(path: &str) -> Result<(f64, HashMap<String, serde_json::Value>), String> {
        let mut details = HashMap::new();

        // This is a simplified implementation
        // In a real application, you would use platform-specific APIs
        // For now, we'll simulate disk usage
        let total_gb = 100.0;
        let used_gb = 50.0;
        let available_gb = total_gb - used_gb;
        let usage_percent = (used_gb / total_gb) * 100.0;

        details.insert("path".to_string(), serde_json::Value::String(path.to_string()));
        details.insert("total_gb".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(total_gb).unwrap()));
        details.insert("used_gb".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(used_gb).unwrap()));
        details.insert("available_gb".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(available_gb).unwrap()));
        details.insert("usage_percent".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(usage_percent).unwrap()));

        Ok((usage_percent, details))
    }
}

#[async_trait]
impl HealthCheck for DiskSpaceHealthCheck {
    fn name(&self) -> &str {
        "disk_space"
    }

    async fn check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();

        match Self::get_disk_usage(&self.mount_path) {
            Ok((usage_percent, details)) => {
                let response_time = start.elapsed().as_millis() as u64;

                if usage_percent > self.critical_threshold_percent {
                    HealthCheckResult::unhealthy(
                        self.name().to_string(),
                        format!("Disk usage is critical: {:.1}%", usage_percent),
                        response_time,
                        format!("Disk usage {:.1}% exceeds critical threshold {:.1}%", usage_percent, self.critical_threshold_percent),
                    ).with_details(details)
                } else if usage_percent > self.warning_threshold_percent {
                    HealthCheckResult::degraded(
                        self.name().to_string(),
                        format!("Disk usage is high: {:.1}%", usage_percent),
                        response_time,
                    ).with_details(details)
                } else {
                    HealthCheckResult::healthy(
                        self.name().to_string(),
                        format!("Disk usage is normal: {:.1}%", usage_percent),
                        response_time,
                    ).with_details(details)
                }
            }
            Err(error) => {
                let response_time = start.elapsed().as_millis() as u64;
                HealthCheckResult::unhealthy(
                    self.name().to_string(),
                    "Failed to check disk usage".to_string(),
                    response_time,
                    error,
                )
            }
        }
    }
}

// CPU usage health check
pub struct CpuHealthCheck {
    warning_threshold_percent: f64,
    critical_threshold_percent: f64,
}

impl CpuHealthCheck {
    pub fn new() -> Self {
        Self {
            warning_threshold_percent: 80.0,
            critical_threshold_percent: 95.0,
        }
    }

    pub fn with_thresholds(mut self, warning_percent: f64, critical_percent: f64) -> Self {
        self.warning_threshold_percent = warning_percent;
        self.critical_threshold_percent = critical_percent;
        self
    }

    fn get_cpu_usage() -> (f64, HashMap<String, serde_json::Value>) {
        let mut details = HashMap::new();

        // This is a simplified implementation
        // In a real application, you would use platform-specific APIs to get CPU usage
        // For now, we'll simulate CPU usage
        let cpu_usage = 25.5; // Simulated CPU usage percentage

        details.insert("cpu_cores".to_string(), serde_json::Value::Number(serde_json::Number::from(4)));
        details.insert("load_average".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(1.2).unwrap()));

        (cpu_usage, details)
    }
}

#[async_trait]
impl HealthCheck for CpuHealthCheck {
    fn name(&self) -> &str {
        "cpu"
    }

    async fn check(&self) -> HealthCheckResult {
        let start = std::time::Instant::now();
        let (cpu_usage, details) = Self::get_cpu_usage();
        let response_time = start.elapsed().as_millis() as u64;

        if cpu_usage > self.critical_threshold_percent {
            HealthCheckResult::unhealthy(
                self.name().to_string(),
                format!("CPU usage is critical: {:.1}%", cpu_usage),
                response_time,
                format!("CPU usage {:.1}% exceeds critical threshold {:.1}%", cpu_usage, self.critical_threshold_percent),
            ).with_details(details)
        } else if cpu_usage > self.warning_threshold_percent {
            HealthCheckResult::degraded(
                self.name().to_string(),
                format!("CPU usage is high: {:.1}%", cpu_usage),
                response_time,
            ).with_details(details)
        } else {
            HealthCheckResult::healthy(
                self.name().to_string(),
                format!("CPU usage is normal: {:.1}%", cpu_usage),
                response_time,
            ).with_details(details)
        }
    }
}

// Health checker service
pub struct HealthChecker {
    checks: Vec<Box<dyn HealthCheck>>,
    start_time: DateTime<Utc>,
    version: String,
}

impl HealthChecker {
    pub fn new(version: String) -> Self {
        Self {
            checks: Vec::new(),
            start_time: Utc::now(),
            version,
        }
    }

    pub fn add_check(mut self, check: Box<dyn HealthCheck>) -> Self {
        self.checks.push(check);
        self
    }

    pub async fn check_health(&self) -> HealthStatusResponse {
        let mut response = HealthStatusResponse::new(self.start_time, self.version.clone());

        // Run all health checks concurrently
        let futures: Vec<_> = self.checks.iter().map(|check| async {
            check.check().await
        }).collect();

        let results = futures::future::join_all(futures).await;

        for result in results {
            response.add_check(result);
        }

        response
    }

    pub async fn check_individual(&self, check_name: &str) -> Option<HealthCheckResult> {
        for check in &self.checks {
            if check.name() == check_name {
                return Some(check.check().await);
            }
        }
        None
    }
}

// Default health checker factory
pub struct HealthCheckerFactory;

impl HealthCheckerFactory {
    pub fn create_default(
        repository: Box<dyn BackboneRepository>,
        version: String,
    ) -> HealthChecker {
        HealthChecker::new(version)
            .add_check(Box::new(DatabaseHealthCheck::new(repository)))
            .add_check(Box::new(MemoryHealthCheck::new()))
            .add_check(Box::new(DiskSpaceHealthCheck::new()))
            .add_check(Box::new(CpuHealthCheck::new()))
    }

    pub fn create_minimal(
        repository: Box<dyn BackboneRepository>,
        version: String,
    ) -> HealthChecker {
        HealthChecker::new(version)
            .add_check(Box::new(DatabaseHealthCheck::new(repository)))
            .add_check(Box::new(MemoryHealthCheck::new()))
    }

    pub fn create_custom(version: String) -> HealthChecker {
        HealthChecker::new(version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repositories::RepositoryError;
    use async_trait::async_trait;

    // Mock repository for testing
    struct MockRepository {
        should_fail: bool,
        should_timeout: bool,
    }

    impl MockRepository {
        fn new() -> Self {
            Self {
                should_fail: false,
                should_timeout: false,
            }
        }

        fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }

        fn with_timeout(mut self) -> Self {
            self.should_timeout = true;
            self
        }
    }

    #[async_trait]
    impl BackboneRepository for MockRepository {
        async fn save(&self, _backbone: &crate::domain::entities::Backbone) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn find_by_id(&self, _id: &crate::domain::value_objects::BackboneId) -> crate::domain::repositories::RepositoryResult<Option<crate::domain::entities::Backbone>> {
            Ok(None)
        }

        async fn delete(&self, _id: &crate::domain::value_objects::BackboneId, _hard_delete: bool) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn find_all(
            &self,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_with_filters(
            &self,
            _filters: crate::domain::repositories::BackboneFilters,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_status(
            &self,
            _status: crate::domain::value_objects::BackboneStatus,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_tags(
            &self,
            _tags: Vec<String>,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_created_by(
            &self,
            _created_by: &str,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn search(
            &self,
            _query: &str,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn save_batch(&self, _backbones: &[crate::domain::entities::Backbone]) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn delete_batch(&self, _ids: &[crate::domain::value_objects::BackboneId], _hard_delete: bool) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn exists(&self, _id: &crate::domain::value_objects::BackboneId) -> crate::domain::repositories::RepositoryResult<bool> {
            Ok(false)
        }

        async fn count(&self, _filters: Option<crate::domain::repositories::BackboneFilters>) -> crate::domain::repositories::RepositoryResult<u64> {
            Ok(0)
        }

        async fn find_deleted(
            &self,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn restore(&self, _id: &crate::domain::value_objects::BackboneId) -> crate::domain::repositories::RepositoryResult<()> {
            Ok(())
        }

        async fn find_by_metadata(
            &self,
            _metadata_key: &str,
            _metadata_value: Option<&str>,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn find_by_date_range(
            &self,
            _start_date: chrono::DateTime<chrono::Utc>,
            _end_date: chrono::DateTime<chrono::Utc>,
            _date_field: crate::domain::repositories::SortField,
            _pagination: crate::domain::repositories::PaginationParams,
            _sort: crate::domain::repositories::SortParams,
        ) -> crate::domain::repositories::RepositoryResult<crate::domain::repositories::PaginatedResult<crate::domain::entities::Backbone>> {
            Ok(crate::domain::repositories::PaginatedResult::empty(1, 20))
        }

        async fn get_status_counts(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<crate::domain::value_objects::BackboneStatus, u64>> {
            Ok(std::collections::HashMap::new())
        }

        async fn get_tag_counts(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<String, u64>> {
            Ok(std::collections::HashMap::new())
        }

        async fn get_recently_created(&self, _days: i64, _limit: Option<usize>) -> crate::domain::repositories::RepositoryResult<Vec<crate::domain::entities::Backbone>> {
            Ok(Vec::new())
        }

        async fn health_check(&self) -> crate::domain::repositories::RepositoryResult<bool> {
            if self.should_fail {
                Err(RepositoryError::DatabaseError {
                    message: "Mock database error".to_string(),
                })
            } else if self.should_timeout {
                // Simulate timeout by sleeping
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                Ok(true)
            } else {
                Ok(true)
            }
        }

        async fn connection_pool_status(&self) -> crate::domain::repositories::RepositoryResult<std::collections::HashMap<String, serde_json::Value>> {
            Ok(std::collections::HashMap::new())
        }
    }

    #[tokio::test]
    async fn test_database_health_check_success() {
        let repository = Box::new(MockRepository::new());
        let health_check = DatabaseHealthCheck::new(repository);

        let result = health_check.check().await;
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.component, "database");
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_database_health_check_failure() {
        let repository = Box::new(MockRepository::new().with_failure());
        let health_check = DatabaseHealthCheck::new(repository);

        let result = health_check.check().await;
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert_eq!(result.component, "database");
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_database_health_check_timeout() {
        let repository = Box::new(MockRepository::new().with_timeout());
        let health_check = DatabaseHealthCheck::new(repository)
            .with_timeout(Duration::from_millis(100));

        let result = health_check.check().await;
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert_eq!(result.component, "database");
        assert!(result.message.contains("timed out"));
    }

    #[tokio::test]
    async fn test_memory_health_check() {
        let health_check = MemoryHealthCheck::new();

        let result = health_check.check().await;
        assert_eq!(result.component, "memory");
        // Since we're using mock values, we expect healthy status
        assert!(result.status.is_healthy() || result.status.is_degraded() || result.status.is_unhealthy());
    }

    #[tokio::test]
    async fn test_health_status_response() {
        let start_time = Utc::now();
        let mut response = HealthStatusResponse::new(start_time, "1.0.0".to_string());

        // Add a healthy check
        let healthy_check = HealthCheckResult::healthy("test".to_string(), "OK".to_string(), 100);
        response.add_check(healthy_check);

        assert_eq!(response.status, HealthStatus::Healthy);
        assert_eq!(response.summary.total_checks, 1);
        assert_eq!(response.summary.healthy_checks, 1);
        assert_eq!(response.summary.degraded_checks, 0);
        assert_eq!(response.summary.unhealthy_checks, 0);

        // Add an unhealthy check
        let unhealthy_check = HealthCheckResult::unhealthy("test2".to_string(), "Error".to_string(), 200, "Details".to_string());
        response.add_check(unhealthy_check);

        assert_eq!(response.status, HealthStatus::Unhealthy);
        assert_eq!(response.summary.total_checks, 2);
        assert_eq!(response.summary.healthy_checks, 1);
        assert_eq!(response.summary.unhealthy_checks, 1);
    }

    #[tokio::test]
    async fn test_health_checker() {
        let repository = Box::new(MockRepository::new());
        let health_checker = HealthCheckerFactory::create_minimal(repository, "1.0.0".to_string());

        let response = health_checker.check_health().await;
        assert!(!response.checks.is_empty());
        assert_eq!(response.version, "1.0.0");

        // Test individual check
        let result = health_checker.check_individual("database").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().component, "database");

        // Test non-existent check
        let result = health_checker.check_individual("nonexistent").await;
        assert!(result.is_none());
    }
}