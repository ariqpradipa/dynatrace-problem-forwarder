use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::error::{ForwarderError, Result};

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub dynatrace: DynatraceConfig,
    pub polling: PollingConfig,
    pub database: DatabaseConfig,
    pub connectors: Vec<ConnectorConfig>,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DynatraceConfig {
    pub base_url: String,
    pub tenant: String,
    pub problem_selector: Option<String>,
    #[serde(skip)]
    pub api_token: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PollingConfig {
    pub interval_seconds: u64,
    pub batch_size: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConnectorConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_method")]
    pub method: HttpMethod,
    pub headers: Option<HashMap<String, String>>,
    pub timeout_seconds: Option<u64>,
    pub retry_attempts: Option<u32>,
    #[serde(default = "default_verify_ssl")]
    pub verify_ssl: bool,
}

fn default_method() -> HttpMethod {
    HttpMethod::Post
}

fn default_verify_ssl() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "pretty".to_string()
}

impl Settings {
    /// Load settings from a YAML file
    pub fn load(config_path: &PathBuf) -> Result<Self> {
        // Check if config file exists
        if !config_path.exists() {
            return Err(ForwarderError::Config(format!(
                "Configuration file not found: {}\n\nPlease create a config.yaml file or specify the path with --config.\nYou can use config.yaml.example as a template.",
                config_path.display()
            )));
        }

        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| ForwarderError::Config(format!(
                "Failed to read config file '{}': {}",
                config_path.display(),
                e
            )))?;

        let mut settings: Settings = serde_yaml::from_str(&config_content)?;

        // Load API token from environment variable
        settings.dynatrace.api_token = std::env::var("DYNATRACE_API_TOKEN")
            .ok()
            .or_else(|| {
                // Try from .env file if not in environment
                dotenv::dotenv().ok();
                std::env::var("DYNATRACE_API_TOKEN").ok()
            });

        // Replace environment variable placeholders in connector headers
        if let Some(connectors) = Some(&mut settings.connectors) {
            for connector in connectors.iter_mut() {
                if let Some(headers) = &mut connector.headers {
                    for (_, value) in headers.iter_mut() {
                        if value.starts_with("${") && value.ends_with("}") {
                            let env_var = &value[2..value.len() - 1];
                            if let Ok(env_value) = std::env::var(env_var) {
                                *value = env_value;
                            }
                        }
                    }
                }
            }
        }

        settings.validate()?;

        Ok(settings)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        // Validate Dynatrace config
        if self.dynatrace.base_url.is_empty() {
            return Err(ForwarderError::Validation(
                "Dynatrace base_url cannot be empty".to_string(),
            ));
        }

        if self.dynatrace.tenant.is_empty() {
            return Err(ForwarderError::Validation(
                "Dynatrace tenant cannot be empty".to_string(),
            ));
        }

        if self.dynatrace.api_token.is_none() {
            return Err(ForwarderError::Validation(
                "DYNATRACE_API_TOKEN environment variable is required".to_string(),
            ));
        }

        // Validate polling config
        if self.polling.interval_seconds == 0 {
            return Err(ForwarderError::Validation(
                "polling.interval_seconds must be greater than 0".to_string(),
            ));
        }

        // Validate connectors
        if self.connectors.is_empty() {
            return Err(ForwarderError::Validation(
                "At least one connector must be configured".to_string(),
            ));
        }

        for connector in &self.connectors {
            if connector.name.is_empty() {
                return Err(ForwarderError::Validation(
                    "Connector name cannot be empty".to_string(),
                ));
            }

            if connector.url.is_empty() {
                return Err(ForwarderError::Validation(
                    format!("Connector '{}' URL cannot be empty", connector.name),
                ));
            }

            if !connector.url.starts_with("http://") && !connector.url.starts_with("https://") {
                return Err(ForwarderError::Validation(
                    format!("Connector '{}' URL must start with http:// or https://", connector.name),
                ));
            }
        }

        Ok(())
    }

    /// Get the full API URL for problems endpoint
    pub fn get_problems_url(&self) -> String {
        let mut url = format!(
            "{}/e/{}/api/v2/problems",
            self.dynatrace.base_url.trim_end_matches('/'),
            self.dynatrace.tenant
        );

        if let Some(selector) = &self.dynatrace.problem_selector {
            url.push_str(&format!("?problemSelector={}", selector));
            url.push_str("&sort=-startTime");
        }

        url
    }
}
