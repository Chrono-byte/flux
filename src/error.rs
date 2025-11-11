use thiserror::Error;

#[derive(Error, Debug)]
pub enum DotfilesError {
    #[error("IO error: {0}\n💡 Hint: Check file permissions and disk space")]
    Io(#[from] std::io::Error),

    #[error(
        "Configuration error: {0}\n💡 Hint: Run `dotfiles-manager validate` to check config integrity"
    )]
    Config(String),

    #[error(
        "Git error: {0}\n💡 Hint: Check repository status with `git status` in the dotfiles repo"
    )]
    Git(#[from] git2::Error),

    #[error(
        "TOML parsing error: {0}\n💡 Hint: Check syntax in ~/.config/dotfiles-manager/config.toml"
    )]
    Toml(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}\n💡 Hint: Invalid configuration structure")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("Path error: {0}\n💡 Hint: Verify that files/directories exist and are accessible")]
    Path(String),

    #[error("Invalid tool '{0}'\n💡 Hint: Tool names must be alphanumeric")]
    InvalidTool(String),

    #[error(
        "Profile not found: {0}\n💡 Hint: Run `dotfiles-manager profile list` to see available profiles"
    )]
    ProfileNotFound(String),

    #[error("Operation cancelled by user")]
    Cancelled,
}

impl DotfilesError {
    /// Provide additional context for the error
    #[allow(dead_code)]
    pub fn with_context(self, context: &str) -> Self {
        match self {
            DotfilesError::Config(msg) => {
                DotfilesError::Config(format!("{}\n  Context: {}", msg, context))
            }
            DotfilesError::Path(msg) => {
                DotfilesError::Path(format!("{}\n  Context: {}", msg, context))
            }
            other => other,
        }
    }
}

pub type Result<T> = std::result::Result<T, DotfilesError>;
