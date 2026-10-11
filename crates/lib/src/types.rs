use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

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

/// OS Enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Os {
  /// Linux operating system
  Linux,
  /// macOS operating system (Darwin)
  Darwin,
  /// Windows operating system
  Windows,
}

impl std::fmt::Display for Os {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    match self {
      Self::Linux => write!(f, "linux"),
      Self::Darwin => write!(f, "darwin"),
      Self::Windows => write!(f, "windows"),
    }
  }
}

impl FromStr for Os {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, String> {
    match s.to_lowercase().as_str() {
      "linux" => Ok(Self::Linux),
      "darwin" => Ok(Self::Darwin),
      "windows" => Ok(Self::Windows),
      _ => Err(format!("Unknown OS: {s}")),
    }
  }
}

/// A single condition that can be evaluated against the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
  /// Match operating system (e.g., "linux", "darwin", "windows")
  Os {
    /// The operating system to match
    value: Os,
  },
  /// Match hostname
  Host {
    /// The hostname to match
    value: String,
  },
  /// Match environment variable with specific value
  EnvVar {
    /// The environment variable name
    name: String,
    /// The expected value
    value: String,
  },
  /// Check if command exists in PATH
  CommandExists {
    /// The command name to check
    command: String,
  },
  /// Check if file or directory exists
  FileExists {
    /// The path to check
    path: String,
  },
  /// Check if environment variable exists (any value)
  EnvVarExists {
    /// The environment variable name
    name: String,
  },
  /// Match CPU architecture (e.g., `"x86_64"`, `"aarch64"`)
  Architecture {
    /// The architecture to match
    value: String,
  },
  /// Match current shell (e.g., "bash", "zsh", "fish")
  Shell {
    /// The shell name to match
    value: String,
  },
  /// Match current username
  User {
    /// The username to match
    username: String,
  },
  /// Check if user is in a specific group
  Group {
    /// The group name to check
    groupname: String,
  },
}

impl Condition {
  /// Get a human-readable description of this condition
  #[must_use]
  pub fn describe(&self) -> String {
    match self {
      Self::Os { value } => format!("OS == {value}"),
      Self::Host { value } => format!("Host == {value}"),
      Self::EnvVar { name, value } => format!("${name} == {value}"),
      Self::CommandExists { command } => {
        format!("command '{command}' exists")
      }
      Self::FileExists { path } => format!("path '{path}' exists"),
      Self::EnvVarExists { name } => format!("${name} is set"),
      Self::Architecture { value } => format!("arch == {value}"),
      Self::Shell { value } => format!("shell == {value}"),
      Self::User { username } => format!("user == {username}"),
      Self::Group { groupname } => format!("group == {groupname}"),
    }
  }
}

/// A group of conditions that must all be true for activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionGroup {
  /// All conditions in this group must evaluate to true
  pub conditions: Vec<Condition>,
}

/// Conditions that control when a config entry should be active.
///
/// All specified condition groups must match for the entry to be included.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConditionSpec {
  /// Modern condition groups (AND together)
  #[serde(default)]
  pub groups: Vec<ConditionGroup>,

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

impl ConditionSpec {
  /// Migrate legacy condition fields to the new groups structure
  pub fn migrate_legacy(&mut self) {
    if !self.groups.is_empty() {
      self.os.clear();
      self.host.clear();
      self.env.clear();
      self.cmd_exists.clear();
      return;
    }

    let mut conditions = Vec::new();

    for os_val in self.os.drain(..) {
      if let Ok(os) = os_val.parse::<Os>() {
        conditions.push(Condition::Os { value: os });
      }
    }

    for host_val in self.host.drain(..) {
      conditions.push(Condition::Host { value: host_val });
    }

    for (name, value) in self.env.drain() {
      conditions.push(Condition::EnvVar { name, value });
    }

    for cmd in self.cmd_exists.drain(..) {
      conditions.push(Condition::CommandExists { command: cmd });
    }

    if !conditions.is_empty() {
      self.groups.push(ConditionGroup { conditions });
    }
  }
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

impl FromStr for SymlinkResolution {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "auto" => Ok(Self::Auto),
      "relative" => Ok(Self::Relative),
      "absolute" => Ok(Self::Absolute),
      "follow" => Ok(Self::Follow),
      "replace" => Ok(Self::Replace),
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
}
