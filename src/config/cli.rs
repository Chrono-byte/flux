//! Type-safe CLI and environment configuration handling
//!
//! This module provides centralized, type-safe configuration management
//! for environment variables and CLI arguments, reducing hardcoding and
//! improving maintainability through validation at startup.

use crate::utils::error::{DotfilesError, Result};
use log::{debug, info, warn};
use std::env;
use std::path::PathBuf;

/// Environment configuration keys
pub mod env_keys {
    pub const CONFIG_FILE: &str = "DOTFILES_CONFIG";
    pub const LOG_LEVEL: &str = "DOTFILES_LOG";
    pub const LOG_FORMAT: &str = "DOTFILES_LOG_FORMAT";
    pub const GIT_USERNAME: &str = "GIT_USERNAME";
    pub const GIT_PASSWORD: &str = "GIT_PASSWORD";
}

/// Type-safe environment configuration
///
/// Loads and validates all environment-dependent configuration at startup.
/// Provides early detection of configuration issues with helpful error messages.
#[derive(Debug, Clone)]
pub struct EnvironmentConfig {
    /// Path to configuration file (from DOTFILES_CONFIG or default locations)
    pub config_file: Option<PathBuf>,

    /// Log level (from DOTFILES_LOG environment variable)
    pub log_level: LogLevel,

    /// Log format (from DOTFILES_LOG_FORMAT environment variable)
    pub log_format: LogFormat,

    /// Git username (from GIT_USERNAME environment variable, for HTTPS auth)
    pub git_username: Option<String>,

    /// Git password (from GIT_PASSWORD environment variable, for HTTPS auth)
    pub git_password: Option<String>,

    /// Whether we're in CI/CD environment
    pub is_ci_environment: bool,
}

/// Log level configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Parse from environment variable string
    pub fn from_env(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(DotfilesError::Config(format!(
                "Invalid log level '{}'. Supported values: trace, debug, info, warn, error",
                s
            ))),
        }
    }

    /// Convert to string for log configuration
    pub fn to_string(self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

/// Log format configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Default,
    Json,
    Compact,
}

impl LogFormat {
    /// Parse from environment variable string
    pub fn from_env(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "default" => Ok(LogFormat::Default),
            "json" => Ok(LogFormat::Json),
            "compact" => Ok(LogFormat::Compact),
            _ => Err(DotfilesError::Config(format!(
                "Invalid log format '{}'. Supported values: default, json, compact",
                s
            ))),
        }
    }

    /// Convert to string
    pub fn to_string(self) -> &'static str {
        match self {
            LogFormat::Default => "default",
            LogFormat::Json => "json",
            LogFormat::Compact => "compact",
        }
    }
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            config_file: None,
            log_level: LogLevel::Info,
            log_format: LogFormat::Default,
            git_username: None,
            git_password: None,
            is_ci_environment: env::var("CI").is_ok() || env::var("CONTINUOUS_INTEGRATION").is_ok(),
        }
    }
}

impl EnvironmentConfig {
    /// Load and validate environment configuration
    ///
    /// This function:
    /// 1. Reads all environment variables
    /// 2. Validates their values
    /// 3. Returns errors with helpful suggestions for invalid values
    /// 4. Logs configuration at startup (debug level)
    ///
    /// # Errors
    ///
    /// Returns an error if any environment variable has an invalid value.
    pub fn load() -> Result<Self> {
        let mut config = EnvironmentConfig::default();

        // Load config file path from environment
        if let Ok(config_path_str) = env::var(env_keys::CONFIG_FILE) {
            let config_path = PathBuf::from(&config_path_str);
            if !config_path.exists() {
                warn!(
                    "Configuration file specified in {} does not exist: {}",
                    env_keys::CONFIG_FILE,
                    config_path.display()
                );
            }
            config.config_file = Some(config_path);
            debug!(
                "Config file from {}: {}",
                env_keys::CONFIG_FILE,
                config_path_str
            );
        }

        // Load log level from environment
        if let Ok(log_level_str) = env::var(env_keys::LOG_LEVEL) {
            config.log_level = LogLevel::from_env(&log_level_str)?;
            debug!(
                "Log level from {}: {}",
                env_keys::LOG_LEVEL,
                config.log_level.to_string()
            );
        }

        // Load log format from environment
        if let Ok(log_format_str) = env::var(env_keys::LOG_FORMAT) {
            config.log_format = LogFormat::from_env(&log_format_str)?;
            debug!(
                "Log format from {}: {}",
                env_keys::LOG_FORMAT,
                config.log_format.to_string()
            );
        }

        // Load git credentials
        if let Ok(username) = env::var(env_keys::GIT_USERNAME) {
            if username.is_empty() {
                return Err(DotfilesError::Config(format!(
                    "{} is set but empty. Please provide a non-empty git username.",
                    env_keys::GIT_USERNAME
                )));
            }
            config.git_username = Some(username);
            debug!(
                "Git username loaded from {} (length: {} chars)",
                env_keys::GIT_USERNAME,
                config.git_username.as_ref().map(|u| u.len()).unwrap_or(0)
            );
        }

        if let Ok(password) = env::var(env_keys::GIT_PASSWORD) {
            if password.is_empty() {
                return Err(DotfilesError::Config(format!(
                    "{} is set but empty. Please provide a non-empty git password or token.",
                    env_keys::GIT_PASSWORD
                )));
            }
            config.git_password = Some(password);
            debug!(
                "Git password loaded from {} (length: {} chars)",
                env_keys::GIT_PASSWORD,
                config.git_password.as_ref().map(|p| p.len()).unwrap_or(0)
            );
        }

        // Detect CI environment
        config.is_ci_environment =
            env::var("CI").is_ok() || env::var("CONTINUOUS_INTEGRATION").is_ok();
        if config.is_ci_environment {
            info!("Running in CI/CD environment");
        }

        Ok(config)
    }

    /// Display configuration summary for debugging
    pub fn display_summary(&self) {
        debug!("Environment Configuration Summary:");
        debug!("  Config file: {:?}", self.config_file);
        debug!("  Log level: {}", self.log_level.to_string());
        debug!("  Log format: {}", self.log_format.to_string());
        debug!(
            "  Git credentials: username={}, password={}",
            self.git_username.is_some(),
            self.git_password.is_some()
        );
        debug!("  CI environment: {}", self.is_ci_environment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_parsing() {
        assert_eq!(LogLevel::from_env("debug").unwrap(), LogLevel::Debug);
        assert_eq!(LogLevel::from_env("DEBUG").unwrap(), LogLevel::Debug);
        assert_eq!(LogLevel::from_env("info").unwrap(), LogLevel::Info);
        assert!(LogLevel::from_env("invalid").is_err());
    }

    #[test]
    fn test_log_format_parsing() {
        assert_eq!(LogFormat::from_env("json").unwrap(), LogFormat::Json);
        assert_eq!(LogFormat::from_env("default").unwrap(), LogFormat::Default);
        assert!(LogFormat::from_env("invalid").is_err());
    }

    #[test]
    fn test_default_config() {
        let config = EnvironmentConfig::default();
        assert_eq!(config.log_level, LogLevel::Info);
        assert_eq!(config.log_format, LogFormat::Default);
        assert_eq!(config.config_file, None);
    }
}
