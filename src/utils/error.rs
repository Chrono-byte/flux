use thiserror::Error;

#[derive(Error, Debug)]
pub enum DotfilesError {
    /// IO errors - includes file system and I/O related failures
    /// Error message already includes helpful context from the operation
    #[error("{0}")]
    Io(#[from] std::io::Error),

    /// Configuration errors - issues with config file or configuration values
    /// Includes specific guidance for resolution
    #[error("{0}")]
    Config(String),

    /// Git operation errors - failures during git commands
    /// Includes diagnostics and debugging steps
    #[error("{0}")]
    Git(#[from] git2::Error),

    /// TOML parsing errors - config file syntax issues
    /// Error message includes file location and hints
    #[error(
        "TOML parsing error in ~/.config/flux/config.toml:\n{0}\n💡 Hint: Check TOML syntax - ensure quotes match, commas are placed correctly, and keys are valid"
    )]
    Toml(#[from] toml::de::Error),

    /// TOML serialization errors - cannot write config
    #[error(
        "Failed to save configuration:\n{0}\n💡 Hint: Configuration structure is invalid or disk is full"
    )]
    TomlSerialize(#[from] toml::ser::Error),

    /// Path-related errors - file operations, symlinks, path validation
    /// Includes specific file paths and resolution guidance
    #[error("{0}")]
    Path(String),

    /// Invalid tool name - tool names must match configuration
    #[error(
        "Invalid tool name: '{0}'\n  Why: Tool names must be alphanumeric and match configured tools\n  💡 Solution: Run `flux validate` to see configured tools"
    )]
    InvalidTool(String),

    /// Profile not found - requested profile does not exist
    /// Includes suggestions for resolution
    #[error("{0}")]
    ProfileNotFound(String),

    /// Operation cancelled - user declined to proceed
    #[error("Operation cancelled by user")]
    Cancelled,
}

impl DotfilesError {
    /// Provide additional context for the error
    #[cfg(test)]
    pub fn with_context(self, context: &str) -> Self {
        match self {
            DotfilesError::Config(msg) => {
                DotfilesError::Config(format!("{}\n  Context: {}", msg, context))
            }
            DotfilesError::Path(msg) => {
                DotfilesError::Path(format!("{}\n  Context: {}", msg, context))
            }
            DotfilesError::Io(e) => DotfilesError::Io(e),
            DotfilesError::Git(e) => DotfilesError::Git(e),
            DotfilesError::Toml(e) => DotfilesError::Toml(e),
            DotfilesError::TomlSerialize(e) => DotfilesError::TomlSerialize(e),
            DotfilesError::InvalidTool(msg) => {
                DotfilesError::InvalidTool(format!("{}\n  Context: {}", msg, context))
            }
            DotfilesError::ProfileNotFound(msg) => {
                DotfilesError::ProfileNotFound(format!("{}\n  Context: {}", msg, context))
            }
            DotfilesError::Cancelled => DotfilesError::Cancelled,
        }
    }
}

pub type Result<T> = std::result::Result<T, DotfilesError>;
