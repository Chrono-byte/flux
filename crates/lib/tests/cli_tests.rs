//! Tests for CLI configuration module

use flux_lib::config::cli::{EnvironmentConfig, LogFormat, LogLevel};

#[test]
fn test_log_level_parsing() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(LogLevel::from_env("debug")?, LogLevel::Debug);
    assert_eq!(LogLevel::from_env("DEBUG")?, LogLevel::Debug);
    assert_eq!(LogLevel::from_env("info")?, LogLevel::Info);
    assert!(LogLevel::from_env("invalid").is_err());
    Ok(())
}

#[test]
fn test_log_format_parsing() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(LogFormat::from_env("json")?, LogFormat::Json);
    assert_eq!(LogFormat::from_env("default")?, LogFormat::Default);
    assert!(LogFormat::from_env("invalid").is_err());
    Ok(())
}

#[test]
fn test_default_config() {
    let config = EnvironmentConfig::default();
    assert_eq!(config.log_level, LogLevel::Info);
    assert_eq!(config.log_format, LogFormat::Default);
    assert_eq!(config.config_file, None);
}
