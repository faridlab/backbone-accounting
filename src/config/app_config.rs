//! Application configuration
//!
//! Loads configuration from YAML files and environment variables.

use serde::Deserialize;
use std::path::Path;

/// Module configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ModuleConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub module: ModuleSettings,
    #[serde(default)]
    pub features: FeatureFlags,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
}

fn default_grpc_port() -> u16 {
    50051
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
}

fn default_max_connections() -> u32 {
    20
}
fn default_min_connections() -> u32 {
    5
}
fn default_connect_timeout() -> u64 {
    30
}
fn default_idle_timeout() -> u64 {
    600
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleSettings {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FeatureFlags {
    #[serde(default)]
    pub soft_delete: bool,
    #[serde(default)]
    pub state_machines: bool,
    #[serde(default)]
    pub workflows: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
}

fn default_log_level() -> String {
    "info".to_string()
}
fn default_log_format() -> String {
    "json".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
        }
    }
}

impl ModuleConfig {
    /// Load configuration from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: ModuleConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration with environment-specific overrides
    pub fn load(env: Option<&str>) -> anyhow::Result<Self> {
        let base_path = "config/application.yml";
        let mut config = Self::from_file(base_path)?;

        if let Some(environment) = env {
            let env_path = format!("config/application-{}.yml", environment);
            if Path::new(&env_path).exists() {
                let env_content = std::fs::read_to_string(&env_path)?;
                let env_config: serde_yaml::Value = serde_yaml::from_str(&env_content)?;
                // Merge env config into base config
                // For now, just reload - proper merge would be more complex
                if let Ok(merged) = serde_yaml::from_value(env_config) {
                    config = merged;
                }
            }
        }

        Ok(config)
    }
}
