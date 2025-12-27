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
    /// Optional conditions controlling when this entry is active
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<ConditionSpec>,

    /// Whether this entry should be treated as secret material.
    ///
    /// Secret entries are candidates for stricter permissions and should not be
    /// displayed in logs or diffs that show contents.
    #[serde(default)]
    pub secret: bool,
    /// Optional logical backend identifier for this secret (e.g., "env", "age").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_backend: Option<String>,
    /// Optional key or handle used by the secret backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_key: Option<String>,
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

/// Hooks configuration for pre/post operations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    /// Commands to run before applying changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_apply: Option<Vec<HookCommand>>,
    /// Commands to run after applying changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_apply: Option<Vec<HookCommand>>,
    /// Commands to run before committing changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_commit: Option<Vec<HookCommand>>,
    /// Commands to run after committing changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_commit: Option<Vec<HookCommand>>,
}

/// A single hook command definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookCommand {
    /// Command to execute (shell command string)
    pub command: String,
    /// Optional conditions that must match for this hook to run
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<ConditionSpec>,
    /// Timeout in seconds (default: 30)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// Conditions that control when a config entry should be active.
///
/// All specified condition groups must match for the entry to be included.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConditionSpec {
    /// Allowed operating systems (e.g., "linux", "darwin").
    ///
    /// If empty, matches all OSes.
    #[serde(default)]
    pub os: Vec<String>,

    /// Allowed hostnames.
    ///
    /// If empty, matches all hosts.
    #[serde(default)]
    pub host: Vec<String>,

    /// Required environment variable values.
    ///
    /// Each key/value pair must match the current process environment.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Commands that must exist in PATH.
    ///
    /// All listed commands must be discoverable for the condition to match.
    #[serde(default)]
    pub cmd_exists: Vec<String>,
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
            _ => Err(format!("Invalid symlink resolution: {s}")),
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
    /// Whether this file is marked as secret in configuration
    pub is_secret: bool,
    /// Optional secret backend identifier (e.g., "env", "age")
    pub secret_backend: Option<String>,
    /// Optional secret key used by the backend
    pub secret_key: Option<String>,
}
