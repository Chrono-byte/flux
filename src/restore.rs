use crate::config::Config;
use crate::error::{DotfilesError, Result};
use chrono::DateTime;
use colored::Colorize;
use dirs;
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
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(timestamp) = chrono::NaiveDateTime::parse_from_str(dir_name, "%Y%m%d_%H%M%S") {
                    // Convert to local timezone
                    let local_timestamp = chrono::DateTime::<chrono::Local>::from_naive_utc_and_offset(
                        timestamp.and_utc().naive_utc(),
                        chrono::Local::now().offset().clone()
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

pub fn restore_backup(backup: &BackupInfo, target_path: &Path) -> Result<()> {
    // Find the corresponding file in backup
    let relative_path = target_path
        .strip_prefix(dirs::home_dir().ok_or_else(|| {
            DotfilesError::Path("Could not find home directory".to_string())
        })?)
        .ok();
    
    let backup_file = if let Some(rel) = relative_path {
        backup.path.join(rel)
    } else {
        // Try to find by filename
        let target_name = target_path.file_name().ok_or_else(|| {
            DotfilesError::Path("Invalid target path".to_string())
        })?;
        
        backup.files.iter()
            .find(|f| f.file_name() == Some(target_name))
            .ok_or_else(|| DotfilesError::Path(format!(
                "File not found in backup: {}",
                target_path.display()
            )))?
            .clone()
    };
    
    if !backup_file.exists() {
        return Err(DotfilesError::Path(format!(
            "Backup file does not exist: {}",
            backup_file.display()
        )));
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
            backup.timestamp.format("%Y-%m-%d %H:%M:%S").to_string().green(),
            backup.files.len()
        );
    }
    
    println!("{}", "=".repeat(60).cyan());
}

