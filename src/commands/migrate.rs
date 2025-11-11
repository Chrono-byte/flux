use crate::commands::untracked::IssueType;
use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::types::{SymlinkResolution, TrackedFile};
use crate::utils::dry_run::DryRun;
use crate::utils::error::{DotfilesError, Result};
use colored::Colorize;
use std::fs;

/// Migrate files to fix discrepancies between tracked files and actual state.
///
/// In dry run mode:
/// - FileSystemManager handles all operations
/// - All file operations are logged but not executed
/// - No files are copied, removed, or symlinked
pub fn migrate_files(
    config: &Config,
    profile: Option<&str>,
    dry_run: &mut DryRun,
    is_dry_run_mode: bool,
) -> Result<()> {
    let mut fs_manager = FileSystemManager::new(dry_run, is_dry_run_mode);

    // Find all discrepancies
    let discrepancies = crate::commands::untracked::find_discrepancies(config, profile)?;

    if discrepancies.is_empty() {
        println!(
            "{} No discrepancies found - all files are correctly configured.",
            "✓".green()
        );
        return Ok(());
    }

    println!(
        "{} Found {} discrepancy(ies) to migrate",
        "→".cyan(),
        discrepancies.len()
    );

    let symlink_resolution = config.general.symlink_resolution;
    let mut migrated_count = 0;
    let mut skipped_count = 0;

    for (idx, discrepancy) in discrepancies.iter().enumerate() {
        println!(
            "\n{} [{}/{}] Migrating: {}",
            "→".cyan(),
            idx + 1,
            discrepancies.len(),
            discrepancy.file.dest_path.display()
        );

        match migrate_file(
            &discrepancy.file,
            &discrepancy.issue,
            &symlink_resolution,
            config,
            &mut fs_manager,
        )? {
            MigrationResult::Migrated => {
                migrated_count += 1;
            }
            MigrationResult::Skipped(reason) => {
                skipped_count += 1;
                println!("  {} Skipped: {}", "⊘".yellow(), reason);
            }
        }
    }

    println!("\n{} Migration complete", "✓".green());
    println!(
        "  {} migrated, {} skipped",
        migrated_count.to_string().green(),
        skipped_count.to_string().yellow()
    );

    Ok(())
}

enum MigrationResult {
    Migrated,
    Skipped(String),
}

fn migrate_file(
    file: &TrackedFile,
    issue: &IssueType,
    resolution: &SymlinkResolution,
    config: &Config,
    fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
    match issue {
        IssueType::Missing => {
            // File doesn't exist - just create symlink (repo should exist)
            if !file.repo_path.exists() {
                return Ok(MigrationResult::Skipped(
                    "Repo file does not exist".to_string(),
                ));
            }

            // Create parent directory if needed (fs_manager handles dry run)
            if let Some(parent) = file.dest_path.parent() {
                fs_manager.create_dir_all(parent)?;
                if !fs_manager.is_dry_run {
                    println!("  {} Created parent directory", "✓".green());
                }
            }

            // Create symlink
            let link_target = compute_link_target(file, resolution)?;
            fs_manager.symlink(&link_target, &file.dest_path)?;
            if !fs_manager.is_dry_run {
                println!("  {} Created symlink", "✓".green());
            }
            Ok(MigrationResult::Migrated)
        }

        IssueType::NotSymlink | IssueType::ContentDiffers => {
            // File exists but isn't a symlink or content differs
            // Strategy: Copy current file to repo, then create symlink

            if !file.dest_path.exists() {
                return Ok(MigrationResult::Skipped(
                    "Destination file does not exist".to_string(),
                ));
            }

            // Backup destination before modifying (fs_manager handles dry run)
            println!("  Creating backup...");
            fs_manager.backup_file(&file.dest_path, config, None)?;

            // Copy current file to repo (fs_manager handles dry run)
            println!("  Copying current file to repo...");
            if let Some(parent) = file.repo_path.parent() {
                fs_manager.create_dir_all(parent)?;
            }

            if file.dest_path.is_dir() {
                fs_manager.copy_dir_all(&file.dest_path, &file.repo_path)?;
            } else {
                fs_manager.copy(&file.dest_path, &file.repo_path)?;
            }
            if !fs_manager.is_dry_run {
                println!("  {} Copied to repo", "✓".green());
            }

            // Remove existing file (fs_manager handles dry run)
            fs_manager.remove_file(&file.dest_path)?;

            // Create symlink (fs_manager handles dry run)
            let link_target = compute_link_target(file, resolution)?;
            fs_manager.symlink(&link_target, &file.dest_path)?;
            if !fs_manager.is_dry_run {
                println!("  {} Created symlink", "✓".green());
            }
            Ok(MigrationResult::Migrated)
        }

        IssueType::WrongTarget | IssueType::BrokenSymlink => {
            // Symlink exists but points to wrong place or is broken
            // Strategy: Read what the symlink currently points to (if accessible), copy that to repo, then fix symlink

            // Try to get the actual file content (what the symlink resolves to, if it exists)
            let source_to_copy = if file.dest_path.is_symlink() {
                // It's a symlink - try to read what it points to
                match fs::read_link(&file.dest_path) {
                    Ok(link_target) => {
                        // Resolve relative symlink targets relative to the symlink's parent directory
                        let resolved_target = if link_target.is_absolute() {
                            link_target
                        } else {
                            file.dest_path
                                .parent()
                                .map(|p| p.join(&link_target))
                                .unwrap_or(link_target.clone())
                        };

                        // Check if the link target exists and has content
                        if resolved_target.exists() {
                            // Check if target is empty
                            let is_empty = if resolved_target.is_file() {
                                fs::metadata(&resolved_target)
                                    .map(|m| m.len() == 0)
                                    .unwrap_or(false)
                            } else {
                                false
                            };

                            if is_empty {
                                // Target exists but is empty - try to read content through the symlink
                                // (in case there's content accessible via the symlink path)
                                if file.dest_path.exists() {
                                    // Try reading through the symlink to see if there's content
                                    match fs::read(&file.dest_path) {
                                        Ok(content) if !content.is_empty() => {
                                            // There's content accessible through the symlink
                                            Some(file.dest_path.clone())
                                        }
                                        _ => {
                                            // Empty or can't read - use the empty target
                                            Some(resolved_target)
                                        }
                                    }
                                } else {
                                    // Can't read through symlink - use empty target
                                    Some(resolved_target)
                                }
                            } else {
                                // Target exists and has content
                                Some(resolved_target)
                            }
                        } else {
                            // Broken symlink - try to read content through the symlink path
                            if file.dest_path.exists() {
                                match fs::read(&file.dest_path) {
                                    Ok(content) if !content.is_empty() => {
                                        // There's content accessible through the symlink
                                        Some(file.dest_path.clone())
                                    }
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        }
                    }
                    Err(_) => None,
                }
            } else if file.dest_path.exists() {
                // It's a real file, use it directly
                Some(file.dest_path.clone())
            } else {
                None
            };

            // Copy current file content to repo if we have a source
            if let Some(source) = &source_to_copy {
                // Check if source is empty
                let source_is_empty = if source.is_file() {
                    fs::metadata(source).map(|m| m.len() == 0).unwrap_or(false)
                } else {
                    false
                };

                if source_is_empty {
                    if !fs_manager.is_dry_run {
                        println!("  {} Source file is empty - nothing to copy", "⊘".yellow());
                    }
                    // Still create the repo file structure even if empty
                    if let Some(parent) = file.repo_path.parent() {
                        fs_manager.create_dir_all(parent)?;
                    }
                    // Create empty file if it doesn't exist (using copy from empty source)
                    if !file.repo_path.exists() {
                        fs_manager.copy(source, &file.repo_path)?;
                    }
                } else {
                    println!("  Creating backup...");
                    fs_manager.backup_file(source, config, None)?;

                    // Copy current file to repo (fs_manager handles dry run)
                    println!("  Copying current file to repo...");
                    if let Some(parent) = file.repo_path.parent() {
                        fs_manager.create_dir_all(parent)?;
                    }

                    if source.is_dir() {
                        fs_manager.copy_dir_all(source, &file.repo_path)?;
                    } else {
                        fs_manager.copy(source, &file.repo_path)?;
                    }
                    if !fs_manager.is_dry_run {
                        println!("  {} Copied to repo", "✓".green());
                    }
                }
            } else if !fs_manager.is_dry_run {
                println!(
                    "  {} Warning: Cannot read source file, repo may be empty",
                    "⚠".yellow()
                );
            }

            // Remove old symlink/file (fs_manager handles dry run)
            fs_manager.remove_file(&file.dest_path)?;

            // Create new symlink (fs_manager handles dry run)
            let link_target = compute_link_target(file, resolution)?;
            fs_manager.symlink(&link_target, &file.dest_path)?;
            if !fs_manager.is_dry_run {
                println!("  {} Fixed symlink", "✓".green());
            }
            Ok(MigrationResult::Migrated)
        }

        IssueType::MissingRepo => {
            // Repo file doesn't exist - can't migrate
            Ok(MigrationResult::Skipped(
                "Repo file does not exist - cannot migrate".to_string(),
            ))
        }
    }
}

fn compute_link_target(
    file: &TrackedFile,
    resolution: &SymlinkResolution,
) -> Result<std::path::PathBuf> {
    Ok(match resolution {
        SymlinkResolution::Auto => {
            // Try relative, fall back to absolute
            pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                .unwrap_or_else(|| file.repo_path.clone())
        }
        SymlinkResolution::Relative => {
            pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                .ok_or_else(|| DotfilesError::Path("Cannot create relative symlink".to_string()))?
        }
        SymlinkResolution::Absolute => file.repo_path.clone(),
        SymlinkResolution::Follow => {
            // For migration, just use relative
            pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                .unwrap_or_else(|| file.repo_path.clone())
        }
        SymlinkResolution::Replace => {
            // Replace mode doesn't use symlinks, but we'll still compute a target
            // (This shouldn't be called in Replace mode, but handle it anyway)
            pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                .unwrap_or_else(|| file.repo_path.clone())
        }
    })
}
