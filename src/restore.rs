use crate::config::Config;
use crate::dry_run::DryRun;
use crate::error::{DotfilesError, Result};
use crate::types::TrackedFile;
use chrono::DateTime;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

pub struct BackupInfo {
    pub path: PathBuf,
    pub timestamp: DateTime<chrono::Local>,
    pub files: Vec<PathBuf>,
}

pub fn list_backups(config: &Config) -> Result<Vec<BackupInfo>> {
    let backup_dir = config.get_backup_dir()?;

    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();

    for entry in fs::read_dir(&backup_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Try to parse timestamp from directory name (format: YYYYMMDD_HHMMSS)
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str())
                && let Ok(timestamp) =
                    chrono::NaiveDateTime::parse_from_str(dir_name, "%Y%m%d_%H%M%S")
            {
                // Convert to local timezone
                let local_timestamp = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
                    timestamp.and_utc().naive_utc(),
                    *chrono::Local::now().offset(),
                );

                let mut files = Vec::new();
                collect_backup_files(&path, &mut files)?;

                backups.push(BackupInfo {
                    path,
                    timestamp: local_timestamp,
                    files,
                });
            }
        }
    }

    // Sort by timestamp, newest first
    backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(backups)
}

fn collect_backup_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            collect_backup_files(&path, files)?;
        }
    }

    Ok(())
}

/// Restore a file from a backup.
///
/// In dry run mode:
/// - Logs all operations (directory creation, file removal, file copy) to the `dry_run` tracker
/// - Returns early without performing any file system operations
/// - No files are modified or restored
pub fn restore_backup(
    backup: &BackupInfo,
    target_path: &Path,
    dry_run: &mut DryRun,
    is_dry_run_mode: bool,
) -> Result<()> {
    use crate::dry_run::Operation;

    // Find the corresponding file in backup
    let relative_path = target_path
        .strip_prefix(
            dirs::home_dir()
                .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?,
        )
        .ok();

    let backup_file = if let Some(rel) = relative_path {
        backup.path.join(rel)
    } else {
        // Try to find by filename
        let target_name = target_path
            .file_name()
            .ok_or_else(|| DotfilesError::Path("Invalid target path".to_string()))?;

        backup
            .files
            .iter()
            .find(|f| f.file_name() == Some(target_name))
            .ok_or_else(|| {
                DotfilesError::Path(format!(
                    "File not found in backup: {}",
                    target_path.display()
                ))
            })?
            .clone()
    };

    if !backup_file.exists() {
        return Err(DotfilesError::Path(format!(
            "Backup file does not exist: {}",
            backup_file.display()
        )));
    }

    if is_dry_run_mode {
        // Log operations that would be performed
        if let Some(parent) = target_path.parent()
            && !parent.exists()
        {
            dry_run.log_operation(Operation::CreateDirectory {
                path: parent.to_path_buf(),
            });
        }

        if target_path.exists() || target_path.is_symlink() {
            dry_run.log_operation(Operation::RemoveFile {
                path: target_path.to_path_buf(),
            });
        }

        // Log restore operation (copy from backup)
        dry_run.log_operation(Operation::CopyFile {
            from: backup_file.clone(),
            to: target_path.to_path_buf(),
        });

        return Ok(());
    }

    // Create parent directory if needed
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Remove existing file/symlink
    if target_path.exists() || target_path.is_symlink() {
        if target_path.is_dir() {
            fs::remove_dir_all(target_path)?;
        } else {
            fs::remove_file(target_path)?;
        }
    }

    // Copy backup file to target
    if backup_file.is_dir() {
        copy_dir_all(&backup_file, target_path)?;
    } else {
        fs::copy(&backup_file, target_path)?;
    }

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_all(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}

pub fn display_backups(backups: &[BackupInfo]) {
    if backups.is_empty() {
        println!("{}", "No backups found.".yellow());
        return;
    }

    println!("\n{}", "Available Backups:".bold().cyan());
    println!("{}", "=".repeat(60).cyan());

    for (i, backup) in backups.iter().enumerate() {
        println!(
            "{}. {} - {} file(s)",
            i + 1,
            backup
                .timestamp
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
                .green(),
            backup.files.len()
        );
    }

    println!("{}", "=".repeat(60).cyan());
}

/// Copy files from a backup to the repository and stage them in git.
///
/// Maps backup files to their corresponding repo locations based on tracked files,
/// copies them to the repo, and stages them for commit.
///
/// In dry run mode:
/// - Logs all operations but does not copy files or stage them
pub fn add_backup_to_repo(
    backup: &BackupInfo,
    config: &Config,
    profile: Option<&str>,
    dry_run: &mut DryRun,
    is_dry_run_mode: bool,
) -> Result<Vec<PathBuf>> {
    use crate::git::{init_repo, stage_changes};
    let tracked_files = config.get_tracked_files(profile)?;
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?;
    let repo_path = config.get_repo_path()?;

    println!("{} Copying files from backup to repository...", "→".cyan());

    let mut copied_files = Vec::new();

    // Build a map of destination paths to tracked files for quick lookup
    let mut dest_to_tracked: std::collections::HashMap<PathBuf, &TrackedFile> =
        std::collections::HashMap::new();
    for tracked in &tracked_files {
        dest_to_tracked.insert(tracked.dest_path.clone(), tracked);
    }

    // Process each file in the backup
    for backup_file in &backup.files {
        // Get relative path from backup directory
        let relative_path = backup_file.strip_prefix(&backup.path).map_err(|_| {
            DotfilesError::Path(format!(
                "Could not compute relative path for backup file: {}",
                backup_file.display()
            ))
        })?;

        // Try to find matching tracked file by destination path
        let dest_path = home.join(relative_path);
        let tracked_file = if let Some(tracked) = dest_to_tracked.get(&dest_path) {
            tracked
        } else {
            // Try to find by filename as fallback
            let file_name = backup_file
                .file_name()
                .ok_or_else(|| DotfilesError::Path("Invalid backup file path".to_string()))?;

            tracked_files
                .iter()
                .find(|t| t.repo_path.file_name() == Some(file_name))
                .ok_or_else(|| {
                    DotfilesError::Path(format!(
                        "No tracked file found for backup file: {}",
                        backup_file.display()
                    ))
                })?
        };

        let repo_target = &tracked_file.repo_path;

        if is_dry_run_mode {
            println!(
                "  [DRY RUN] Would copy {} -> {}",
                backup_file.display(),
                repo_target.display()
            );
            copied_files.push(repo_target.clone());
        } else {
            // Create parent directory if needed
            if let Some(parent) = repo_target.parent() {
                fs::create_dir_all(parent)?;
            }

            // Copy file or directory
            if backup_file.is_dir() {
                copy_dir_all(backup_file, repo_target)?;
            } else {
                fs::copy(backup_file, repo_target)?;
            }

            println!(
                "  {} Copied {} -> {}",
                "✓".green(),
                backup_file.display(),
                repo_target.display()
            );
            copied_files.push(repo_target.clone());
        }
    }

    if !is_dry_run_mode && !copied_files.is_empty() {
        // Stage the files in git
        println!("\n{} Staging files in git...", "→".cyan());
        let repo = init_repo(&repo_path)?;

        // Detect all changes in the repo (this will include our copied files)
        let changes = crate::git::detect_changes(&repo)?;

        if !changes.is_empty() {
            stage_changes(&repo, &changes, dry_run, is_dry_run_mode)?;
            println!("{} Staged {} file(s)", "✓".green(), changes.len());
        } else {
            println!("  {} No changes detected to stage", "⊘".yellow());
        }
    }

    println!(
        "\n{} Added {} file(s) from backup to repository",
        "✓".green(),
        copied_files.len()
    );

    Ok(copied_files)
}
