use crate::config::Config;
use crate::dry_run::{DryRun, Operation};
use crate::error::{DotfilesError, Result};
use crate::prompt::{prompt_conflict, ConflictResolution};
use crate::types::{SymlinkResolution, TrackedFile};
use chrono::Local;
use colored::Colorize;
use fslock::LockFile;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

// Track if we're in dry run mode
static mut DRY_RUN_MODE: bool = false;

pub fn set_dry_run_mode(enabled: bool) {
    unsafe {
        DRY_RUN_MODE = enabled;
    }
}

pub fn is_dry_run() -> bool {
    unsafe { DRY_RUN_MODE }
}

pub fn is_file_locked(file_path: &Path) -> bool {
    // Try to acquire a lock on the file
    if let Ok(mut lock) = LockFile::open(file_path) {
        if lock.try_lock().is_err() {
            return true; // File is locked
        }
        let _ = lock.unlock();
    }
    false
}

pub fn add_file(
    config: &mut Config,
    tool: &str,
    source_path: &Path,
    dest_path: &Path,
    profile: Option<&str>,
) -> Result<()> {
    // Check if source file is locked
    if is_file_locked(source_path) {
        return Err(DotfilesError::Path(format!(
            "Source file {} is locked (may be in use), cannot add",
            source_path.display()
        )));
    }
    
    let repo_path = config.get_repo_path()?;
    let tool_dir = repo_path.join(tool);
    fs::create_dir_all(&tool_dir)?;

    let repo_file = tool_dir.join(
        source_path
            .file_name()
            .ok_or_else(|| DotfilesError::Path("Invalid source path".to_string()))?,
    );

    // Copy file or directory to repo
    if source_path.is_dir() {
        copy_dir_all(source_path, &repo_file)?;
    } else {
        fs::copy(source_path, &repo_file)?;
    }

    // Add to config
    let repo_relative = repo_file
        .strip_prefix(&repo_path)
        .map_err(|_| DotfilesError::Path("Could not compute relative path".to_string()))?
        .to_string_lossy()
        .to_string();
    let dest_str = dest_path.to_string_lossy().to_string();
    config.add_file_to_tool(tool, &repo_relative, &dest_str, profile)?;
    config.save()?;

    println!(
        "{} Added {} to {} tool",
        "✓".green(),
        source_path.display(),
        tool
    );

    Ok(())
}

pub fn sync_files(config: &Config, profile: Option<&str>, dry_run: &mut DryRun, is_dry_run_mode: bool) -> Result<()> {
    set_dry_run_mode(is_dry_run_mode);
    let tracked_files = config.get_tracked_files(profile)?;
    let symlink_resolution = config.get_symlink_resolution()?;

    for file in tracked_files {
        sync_file(&file, &symlink_resolution, dry_run)?;
    }

    Ok(())
}

fn sync_file(
    file: &TrackedFile,
    resolution: &SymlinkResolution,
    dry_run: &mut DryRun,
) -> Result<()> {
    // Check if file is locked
    if is_file_locked(&file.dest_path) {
        println!(
            "{} Warning: {} is locked (may be in use), skipping",
            "⚠".yellow(),
            file.dest_path.display()
        );
        return Ok(());
    }
    
    if is_file_locked(&file.repo_path) {
        println!(
            "{} Warning: {} is locked (may be in use), skipping",
            "⚠".yellow(),
            file.repo_path.display()
        );
        return Ok(());
    }
    
    // Check if destination exists
    if file.dest_path.exists() {
        // Check if it's already a symlink pointing to the right place
        if let Ok(link_target) = fs::read_link(&file.dest_path) {
            if link_target == file.repo_path {
                // Already correctly linked
                return Ok(());
            }
        }

        // Check if files are different
        if files_differ(&file.repo_path, &file.dest_path)? {
            let conflict_resolution = prompt_conflict(&file.dest_path)?;
            match conflict_resolution {
                ConflictResolution::BackupAndReplace => {
                    create_backup(&file.dest_path, dry_run)?;
                    create_symlink(file, resolution, dry_run)?;
                }
                ConflictResolution::Skip => {
                    println!("{} Skipped {}", "⊘".yellow(), file.dest_path.display());
                    return Ok(());
                }
                ConflictResolution::ViewDiff => {
                    show_diff(&file.repo_path, &file.dest_path)?;
                    // Ask again after showing diff
                    let conflict_resolution = prompt_conflict(&file.dest_path)?;
                    match conflict_resolution {
                        ConflictResolution::BackupAndReplace => {
                            create_backup(&file.dest_path, dry_run)?;
                            create_symlink(file, resolution, dry_run)?;
                        }
                        ConflictResolution::Skip => {
                            println!("{} Skipped {}", "⊘".yellow(), file.dest_path.display());
                            return Ok(());
                        }
                        ConflictResolution::Cancel => {
                            return Err(DotfilesError::Cancelled);
                        }
                        _ => {}
                    }
                }
                ConflictResolution::Cancel => {
                    return Err(DotfilesError::Cancelled);
                }
            }
        } else {
            // Files are the same, just ensure symlink exists
            if !file.dest_path.exists() || fs::read_link(&file.dest_path).is_err() {
                create_symlink(file, resolution, dry_run)?;
            }
        }
    } else {
        // Destination doesn't exist, create parent dirs and symlink
        if let Some(parent) = file.dest_path.parent() {
            if !parent.exists() {
                let is_dry_run = is_dry_run();
                if !is_dry_run && !dry_run.is_empty() {
                    dry_run.log_operation(Operation::CreateDirectory {
                        path: parent.to_path_buf(),
                    });
                } else if !is_dry_run {
                    fs::create_dir_all(parent)?;
                } else {
                    dry_run.log_operation(Operation::CreateDirectory {
                        path: parent.to_path_buf(),
                    });
                }
            }
        }
        create_symlink(file, resolution, dry_run)?;
    }

    Ok(())
}

fn create_symlink(
    file: &TrackedFile,
    resolution: &SymlinkResolution,
    dry_run: &mut DryRun,
) -> Result<()> {
    let is_dry_run = is_dry_run();
    
    if !is_dry_run && !dry_run.is_empty() {
        // In dry run mode, just log
        dry_run.log_operation(Operation::CreateSymlink {
            from: file.repo_path.clone(),
            to: file.dest_path.clone(),
        });
        return Ok(());
    }
    
    if !is_dry_run {
        // Remove existing file/symlink if it exists
        if file.dest_path.exists() || file.dest_path.is_symlink() {
            fs::remove_file(&file.dest_path)?;
        }

        let link_target = match resolution {
            SymlinkResolution::Auto => {
                // Try relative, fall back to absolute
                pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                    .unwrap_or_else(|| file.repo_path.clone())
            }
            SymlinkResolution::Relative => {
                pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                    .ok_or_else(|| {
                        DotfilesError::Path("Cannot create relative symlink".to_string())
                    })?
            }
            SymlinkResolution::Absolute => file.repo_path.clone(),
            SymlinkResolution::Follow => {
                // Follow existing symlink if it exists
                if file.dest_path.is_symlink() {
                    let target = fs::read_link(&file.dest_path)?;
                    if target.exists() {
                        fs::remove_file(&target)?;
                    }
                }
                pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                    .unwrap_or_else(|| file.repo_path.clone())
            }
            SymlinkResolution::Replace => {
                // Copy file instead of symlinking
                fs::copy(&file.repo_path, &file.dest_path)?;
                println!(
                    "{} Copied {} -> {}",
                    "✓".green(),
                    file.repo_path.display(),
                    file.dest_path.display()
                );
                return Ok(());
            }
        };

        symlink(&link_target, &file.dest_path)?;
        println!(
            "{} Linked {} -> {}",
            "✓".green(),
            file.repo_path.display(),
            file.dest_path.display()
        );
    }

    Ok(())
}

fn create_backup(file_path: &Path, dry_run: &mut DryRun) -> Result<PathBuf> {
    // This would need config, but for now use a simple approach
    let backup_dir = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?
        .join(".dotfiles")
        .join(".backups")
        .join(Local::now().format("%Y%m%d_%H%M%S").to_string());

    let relative_path = file_path
        .strip_prefix(dirs::home_dir().unwrap())
        .unwrap_or(file_path);
    let backup_path = backup_dir.join(relative_path);

    let is_dry_run = is_dry_run();
    if !is_dry_run {
        fs::create_dir_all(backup_path.parent().unwrap())?;
        fs::copy(file_path, &backup_path)?;
        println!(
            "{} Backed up {} -> {}",
            "✓".yellow(),
            file_path.display(),
            backup_path.display()
        );
    } else {
        dry_run.log_operation(Operation::CreateBackup {
            file: file_path.to_path_buf(),
            backup: backup_path.clone(),
        });
    }

    Ok(backup_path)
}

fn files_differ(path1: &Path, path2: &Path) -> Result<bool> {
    if !path1.exists() || !path2.exists() {
        return Ok(true);
    }

    let content1 = fs::read(path1)?;
    let content2 = fs::read(path2)?;

    Ok(content1 != content2)
}

fn show_diff(path1: &Path, path2: &Path) -> Result<()> {
    use std::process::Command;

    let output = Command::new("diff")
        .arg("-u")
        .arg(path1)
        .arg(path2)
        .output()?;

    if output.status.success() || output.status.code() == Some(1) {
        print!("{}", String::from_utf8_lossy(&output.stdout));
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

pub fn remove_file(
    config: &mut Config,
    tool: &str,
    file: &str,
    dry_run: &mut DryRun,
) -> Result<()> {
    // Find the file in config
    let tool_config = config
        .tools
        .get(tool)
        .ok_or_else(|| DotfilesError::InvalidTool(tool.to_string()))?;

    let file_entry = tool_config
        .files
        .iter()
        .find(|e| e.repo == file)
        .ok_or_else(|| DotfilesError::Path(format!("File {} not found in tool {}", file, tool)))?;

    let dest_path = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?
        .join(&file_entry.dest);

    // Remove symlink
    if dest_path.exists() || dest_path.is_symlink() {
        let is_dry_run = is_dry_run();
        if !is_dry_run {
            fs::remove_file(&dest_path)?;
        } else {
            dry_run.log_operation(Operation::RemoveFile {
                path: dest_path.clone(),
            });
        }
    }

    // Remove from repo
    let repo_path = config.get_repo_path()?;
    let repo_file = repo_path.join(tool).join(file);
    if repo_file.exists() {
        let is_dry_run = is_dry_run();
        if !is_dry_run {
            fs::remove_file(&repo_file)?;
        } else {
            dry_run.log_operation(Operation::RemoveFile {
                path: repo_file.clone(),
            });
        }
    }

    // Remove from config
    if let Some(tool_config) = config.tools.get_mut(tool) {
        tool_config.files.retain(|e| e.repo != file);
        if tool_config.files.is_empty() {
            config.tools.remove(tool);
        }
    }

    config.save()?;

    println!("{} Removed {} from {} tool", "✓".green(), file, tool);

    Ok(())
}

