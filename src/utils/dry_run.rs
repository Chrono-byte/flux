use colored::Colorize;
use std::path::PathBuf;

/// An operation that can be logged in dry-run mode.
#[derive(Debug, Clone)]
pub enum Operation {
    /// Create a symlink
    CreateSymlink { from: PathBuf, to: PathBuf },
    /// Create a backup of a file
    CreateBackup { file: PathBuf, backup: PathBuf },
    /// Create a directory
    CreateDirectory { path: PathBuf },
    /// Copy a file
    CopyFile { from: PathBuf, to: PathBuf },
    /// Remove a file
    RemoveFile { path: PathBuf },
    /// Git commit operation
    GitCommit { message: String },
    /// Git stage operation
    GitStage { files: Vec<PathBuf> },
    /// Add a git remote
    GitRemoteAdd { name: String, url: String },
    /// Remove a git remote
    GitRemoteRemove { name: String },
    /// Set URL for a git remote
    GitRemoteSetUrl { name: String, url: String },
    /// Push to git remote
    GitPush {
        remote: String,
        branch: String,
        set_upstream: bool,
    },
    /// Pull from git remote
    GitPull { remote: String, branch: String },
}

/// Tracks operations for dry-run mode.
pub struct DryRun {
    operations: Vec<Operation>,
}

impl DryRun {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }

    pub fn log_operation(&mut self, operation: Operation) {
        self.operations.push(operation);
    }

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
            println!("\n{}. {}", i + 1, format!("{:?}", op).bright_white());
            match op {
                Operation::CreateSymlink { from, to } => {
                    println!(
                        "   {} {} -> {}",
                        "Create symlink:".green(),
                        from.display(),
                        to.display()
                    );
                }
                Operation::CreateBackup { file, backup } => {
                    println!(
                        "   {} {} -> {}",
                        "Create backup:".yellow(),
                        file.display(),
                        backup.display()
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
}

impl Default for DryRun {
    fn default() -> Self {
        Self::new()
    }
}
