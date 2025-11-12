use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A tracked file entry in the configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Repository path relative to repo root
    pub repo: String,
    /// Destination path relative to home directory
    pub dest: String,
    /// Optional profile name for this file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Represents a change detected in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    /// File was added
    Added(PathBuf),
    /// File was modified
    Modified(PathBuf),
    /// File was deleted
    Deleted(PathBuf),
}

// ==================== Environment Types ====================

/// Environment configuration for declarative operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    /// Environment variables to set
    #[serde(default)]
    pub variables: HashMap<String, String>,

    /// Shell to use for command execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
}

/// Strategy for resolving symlink targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymlinkResolution {
    /// Automatically choose relative or absolute based on path
    Auto,
    /// Always use relative paths
    Relative,
    /// Always use absolute paths
    Absolute,
    /// Follow existing symlinks (treated as Auto)
    Follow,
    /// Copy files instead of creating symlinks
    Replace,
}

impl std::str::FromStr for SymlinkResolution {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(SymlinkResolution::Auto),
            "relative" => Ok(SymlinkResolution::Relative),
            "absolute" => Ok(SymlinkResolution::Absolute),
            "follow" => Ok(SymlinkResolution::Follow),
            "replace" => Ok(SymlinkResolution::Replace),
            _ => Err(format!("Invalid symlink resolution: {}", s)),
        }
    }
}

/// A file being tracked by the dotfiles manager.
#[derive(Debug, Clone)]
pub struct TrackedFile {
    /// Tool name this file belongs to
    pub tool: String,
    /// Full path to the file in the repository
    pub repo_path: PathBuf,
    /// Full path to the file in the home directory
    pub dest_path: PathBuf,
    /// Optional profile name for this file
    pub profile: Option<String>,
}
