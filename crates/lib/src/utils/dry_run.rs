use colored::Colorize;
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
  /// Operations to perform.
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

  /// Display a summary of all logged operations.
  pub fn display_summary(&self) {
    if self.operations.is_empty() {
      println!("{}", "No operations to perform.".yellow());
      return;
    }

    println!(
      "\n{}",
      "DRY RUN - Operations that would be performed:"
        .bold()
        .cyan()
    );
    println!("{}", "=".repeat(60).cyan());

    for (i, op) in self.operations.iter().enumerate() {
      println!("\n{}. {}", i + 1, format!("{op:?}").bright_white());
      match op {
        Operation::CreateSymlink { from, to } => {
          println!(
            "   {} {} -> {}",
            "Create symlink:".green(),
            from.display(),
            to.display()
          );
        }
        Operation::CreateDirectory { path } => {
          println!("   {} {}", "Create directory:".blue(), path.display());
        }
        Operation::CopyFile { from, to } => {
          println!(
            "   {} {} -> {}",
            "Copy file:".cyan(),
            from.display(),
            to.display()
          );
        }
        Operation::RemoveFile { path } => {
          println!("   {} {}", "Remove file:".red(), path.display());
        }
        Operation::GitCommit { message } => {
          println!("   {} {}", "Git commit:".magenta(), message);
        }
        Operation::GitStage { files } => {
          println!("   {} {} file(s)", "Git stage:".magenta(), files.len());
          for file in files {
            println!("      - {}", file.display());
          }
        }
        Operation::GitRemoteAdd { name, url } => {
          println!(
            "   {} Add remote '{}': {}",
            "Git remote:".bright_magenta(),
            name.cyan(),
            url
          );
        }
        Operation::GitRemoteRemove { name } => {
          println!(
            "   {} Remove remote '{}'",
            "Git remote:".bright_magenta(),
            name.cyan()
          );
        }
        Operation::GitRemoteSetUrl { name, url } => {
          println!(
            "   {} Set URL for remote '{}': {}",
            "Git remote:".bright_magenta(),
            name.cyan(),
            url
          );
        }
        Operation::GitPush {
          remote,
          branch,
          set_upstream,
        } => {
          println!(
            "   {} Push branch '{}' to remote '{}' (set_upstream: {})",
            "Git push:".bright_magenta(),
            branch.cyan(),
            remote.cyan(),
            if *set_upstream {
              "yes".green()
            } else {
              "no".yellow()
            }
          );
        }
        Operation::GitPull { remote, branch } => {
          println!(
            "   {} Pull branch '{}' from remote '{}'",
            "Git pull:".bright_magenta(),
            branch.cyan(),
            remote.cyan()
          );
        }
      }
    }

    println!("\n{}", "=".repeat(60).cyan());
    println!(
      "{}",
      format!("Total operations: {}", self.operations.len()).bold()
    );
  }

  /// Returns the operations that have been logged.
  #[cfg_attr(not(test), allow(dead_code))]
  pub fn operations(&self) -> &[Operation] {
    &self.operations
  }
}

impl Default for DryRun {
  fn default() -> Self {
    Self::new()
  }
}
