use colored::Colorize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum Operation {
    CreateSymlink {
        from: PathBuf,
        to: PathBuf,
    },
    CreateBackup {
        file: PathBuf,
        backup: PathBuf,
    },
    CreateDirectory {
        path: PathBuf,
    },
    CopyFile {
        from: PathBuf,
        to: PathBuf,
    },
    RemoveFile {
        path: PathBuf,
    },
    GitCommit {
        message: String,
    },
    GitStage {
        files: Vec<PathBuf>,
    },
    GitRemoteAdd {
        name: String,
        url: String,
    },
    GitRemoteRemove {
        name: String,
    },
    GitRemoteSetUrl {
        name: String,
        url: String,
    },
    GitPush {
        remote: String,
        branch: String,
        set_upstream: bool,
    },
}

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
