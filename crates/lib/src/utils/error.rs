use thiserror::Error;

/// Error type for the Flux dotfiles manager.
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

  /// JSON serialization errors - cannot serialize to JSON
  #[error("Failed to serialize to JSON: {0}")]
  JsonSerialize(#[from] serde_json::Error),

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
  #[must_use]
  pub fn with_context(
    self,
    context: &str,
  ) -> Self {
    match self {
      Self::Config(msg) => Self::Config(format!("{msg}\n  Context: {context}")),
      Self::Path(msg) => Self::Path(format!("{msg}\n  Context: {context}")),
      Self::Io(error) => Self::Io(error),
      Self::Git(error) => Self::Git(error),
      Self::Toml(error) => Self::Toml(error),
      Self::TomlSerialize(error) => Self::TomlSerialize(error),
      Self::JsonSerialize(error) => Self::JsonSerialize(error),
      Self::InvalidTool(msg) => Self::InvalidTool(format!("{msg}\n  Context: {context}")),
      Self::ProfileNotFound(msg) => Self::ProfileNotFound(format!("{msg}\n  Context: {context}")),
      Self::Cancelled => Self::Cancelled,
    }
  }
}

/// Result type for the Flux dotfiles manager.
///
/// # Parameters
///
/// * `T`: The type of the result.
///
/// # Returns
///
/// Returns the result.
pub type Result<T> = std::result::Result<T, DotfilesError>;
