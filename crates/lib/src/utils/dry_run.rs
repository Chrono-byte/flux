use std::path::PathBuf;

/// An operation that can be logged in dry-run mode.
#[derive(Debug, Clone)]
pub enum Operation {
  /// Create a symlink
  CreateSymlink {
    /// Source path for the symlink.
    from: PathBuf,
    /// Destination path for the symlink.
    to: PathBuf,
  },
  /// Create a directory
  CreateDirectory {
    /// Path of the directory to create.
    path: PathBuf,
  },
  /// Copy a file
  CopyFile {
    /// Source file path.
    from: PathBuf,
    /// Destination file path.
    to: PathBuf,
  },
  /// Rename or move a file or directory.
  Rename {
    /// Source path.
    from: PathBuf,
    /// Destination path.
    to: PathBuf,
  },
  /// Remove a file
  RemoveFile {
    /// Path of the file to remove.
    path: PathBuf,
  },
  /// Git commit operation
  GitCommit {
    /// Commit message.
    message: String,
  },
  /// Git stage operation
  GitStage {
    /// Files to stage.
    files: Vec<PathBuf>,
  },
  /// Add a git remote
  GitRemoteAdd {
    /// Remote name.
    name: String,
    /// Remote URL.
    url: String,
  },
  /// Remove a git remote
  GitRemoteRemove {
    /// Remote name to remove.
    name: String,
  },
  /// Set URL for a git remote
  GitRemoteSetUrl {
    /// Remote name.
    name: String,
    /// New remote URL.
    url: String,
  },
  /// Push to git remote
  GitPush {
    /// Remote name.
    remote: String,
    /// Branch name.
    branch: String,
    /// Whether to set upstream.
    set_upstream: bool,
  },
  /// Pull from git remote
  GitPull {
    /// Remote name.
    remote: String,
    /// Branch name.
    branch: String,
  },
}

/// Tracks operations for dry-run mode.
#[derive(Debug)]
pub struct DryRun {
  operations: Vec<Operation>,
}

impl DryRun {
  /// Create a new dry-run tracker.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      operations: Vec::new(),
    }
  }

  /// Log an operation for dry-run display.
  pub fn log_operation(
    &mut self,
    operation: Operation,
  ) {
    self.operations.push(operation);
  }

  /// Returns the operations that have been logged.
  #[cfg(not(test))]
  #[must_use = "operations should not be ignored in tests"]
  pub fn operations(&self) -> &[Operation] {
    &self.operations
  }
}

impl Default for DryRun {
  fn default() -> Self {
    Self::new()
  }
}
