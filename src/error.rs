use thiserror::Error;

#[derive(Error, Debug)]
pub enum DotfilesError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Invalid tool: {0}")]
    InvalidTool(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Operation cancelled by user")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, DotfilesError>;

