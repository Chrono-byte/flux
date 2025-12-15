//! Tests for CLI configuration module

use flux::config::cli::{EnvironmentConfig, LogFormat, LogLevel};

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
