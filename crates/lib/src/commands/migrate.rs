use crate::commands::untracked::IssueType;
use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::types::{SymlinkResolution, TrackedFile};
use crate::utils::dry_run::DryRun;
use crate::utils::error::{DotfilesError, Result};
use std::fs;

/// Summary of a migrate operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MigrateReport {
  /// Number of discrepancies found.
  pub discrepancies: usize,
  /// Number of files migrated.
  pub migrated: usize,
  /// Number of discrepancies skipped.
  pub skipped: usize,
}

/// Migrate files to fix discrepancies between tracked files and actual state.
///
/// In dry run mode:
/// - `FileSystemManager` handles all operations
/// - All file operations are logged but not executed
/// - No files are copied, removed, or symlinked
///
/// # Errors
///
/// Returns an error if there are issues reading/writing files or if
/// the configuration is invalid.
pub fn migrate_files(
  config: &Config,
  profile: Option<&str>,
  dry_run: &mut DryRun,
  is_dry_run_mode: bool,
) -> Result<MigrateReport> {
  let mut fs_manager = FileSystemManager::new(dry_run, is_dry_run_mode);

  // Find all discrepancies
  let discrepancies = crate::commands::untracked::find_discrepancies(config, profile)?;

  if discrepancies.is_empty() {
    return Ok(MigrateReport::default());
  }

  let symlink_resolution = config.general.symlink_resolution;
  let mut report = MigrateReport {
    discrepancies: discrepancies.len(),
    ..MigrateReport::default()
  };

  for discrepancy in &discrepancies {
    match migrate_file(
      &discrepancy.file,
      discrepancy.issue,
      symlink_resolution,
      &mut fs_manager,
    )? {
      MigrationResult::Migrated => report.migrated += 1,
      MigrationResult::Skipped => report.skipped += 1,
    }
  }

  Ok(report)
}

/// The result of migrating a file.
enum MigrationResult {
  /// The file was migrated
  Migrated,
  /// The file was skipped
  Skipped,
}

/// Migrate a file.
///
/// # Errors
///
/// Returns an error if there are issues reading/writing files or if
/// the configuration is invalid.
fn migrate_file(
  file: &TrackedFile,
  issue: IssueType,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
  match issue {
    IssueType::Missing => migrate_missing_file(file, resolution, fs_manager),

    IssueType::NotSymlink | IssueType::ContentDiffers => {
      migrate_not_symlink_or_differs(file, resolution, fs_manager)
    }

    IssueType::WrongTarget | IssueType::BrokenSymlink => {
      migrate_wrong_target_or_broken(file, resolution, fs_manager)
    }
    IssueType::MissingRepo => {
      // Repo file doesn't exist - can't migrate
      Ok(MigrationResult::Skipped)
    }
  }
}

/// Migrate a missing file.
///
/// # Errors
///
/// Returns an error if there are issues reading/writing files or if
/// the configuration is invalid.
fn migrate_missing_file(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
  // File doesn't exist - just create symlink (repo should exist)
  if !file.repo_path.exists() {
    return Ok(MigrationResult::Skipped);
  }

  // Create parent directory if needed (fs_manager handles dry run)
  if let Some(parent) = file.dest_path.parent() {
    fs_manager.create_dir_all(parent)?;
  }

  // Create symlink
  let link_target = compute_link_target(file, resolution)?;
  fs_manager.symlink(&link_target, &file.dest_path)?;
  Ok(MigrationResult::Migrated)
}

/// Migrate a not symlink or differs file.
///
/// # Errors
///
/// Returns an error if there are issues reading/writing files or if
/// the configuration is invalid.
fn migrate_not_symlink_or_differs(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
  // File exists but isn't a symlink or content differs
  // Strategy: Copy current file to repo, then create symlink

  if !file.dest_path.exists() {
    return Ok(MigrationResult::Skipped);
  }

  if let Some(parent) = file.repo_path.parent() {
    fs_manager.create_dir_all(parent)?;
  }

  if file.dest_path.is_dir() {
    fs_manager.copy_dir_all(&file.dest_path, &file.repo_path)?;
  } else {
    fs_manager.copy(&file.dest_path, &file.repo_path)?;
  }

  // Common ending: remove existing file and create symlink
  // Remove existing file (fs_manager handles dry run)
  fs_manager.remove_file(&file.dest_path)?;

  // Create symlink (fs_manager handles dry run)
  let link_target = compute_link_target(file, resolution)?;
  fs_manager.symlink(&link_target, &file.dest_path)?;
  Ok(MigrationResult::Migrated)
}

/// Migrate a wrong target or broken file.
///
/// # Errors
///
/// Returns an error if there are issues reading/writing files or if
/// the configuration is invalid.
fn migrate_wrong_target_or_broken(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
  // Symlink exists but points to wrong place or is broken
  // Strategy: Read what the symlink currently points to (if accessible), copy that to repo, then fix symlink

  // Try to get the actual file content (what the symlink resolves to, if it exists)
  let source_to_copy = find_source_to_copy(file)?;

  // Normal flow: copy current content to repo if available
  if let Some(source) = &source_to_copy {
    copy_source_to_repo(source, file, fs_manager)?;
  }

  // Common ending: remove old symlink/file and create new symlink
  // Remove old symlink/file (fs_manager handles dry run)
  fs_manager.remove_file(&file.dest_path)?;

  // Create new symlink (fs_manager handles dry run)
  let link_target = compute_link_target(file, resolution)?;
  fs_manager.symlink(&link_target, &file.dest_path)?;
  Ok(MigrationResult::Migrated)
}

/// Find the source to copy.
///
/// # Returns
///
/// Returns the source to copy if it exists, otherwise None.
fn find_source_to_copy(file: &TrackedFile) -> Result<Option<std::path::PathBuf>> {
  if file.dest_path.is_symlink() {
    // It's a symlink - try to read what it points to
    let link_target = fs::read_link(&file.dest_path)?;
    Ok({
      // Resolve relative symlink targets relative to symlink's parent directory
      let resolved_target = if link_target.is_absolute() {
        link_target
      } else {
        match file.dest_path.parent() {
          Some(parent) => parent.join(&link_target),
          None => link_target,
        }
      };

      // Check if the link target exists and has content
      if resolved_target.exists() {
        // Check if target is empty
        let is_empty = if resolved_target.is_file() {
          fs::metadata(&resolved_target)?.len() == 0
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
    })
  } else if file.dest_path.exists() {
    // It's a real file, use it directly
    Ok(Some(file.dest_path.clone()))
  } else {
    Ok(None)
  }
}

/// Copy the source to the repository.
///
/// # Errors
///
/// Returns an error if there are issues reading/writing files or if
/// the configuration is invalid.
fn copy_source_to_repo(
  source: &std::path::Path,
  file: &TrackedFile,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  // Check if source is empty
  let source_is_empty = if source.is_file() {
    fs::metadata(source)?.len() == 0
  } else {
    false
  };

  if source_is_empty {
    // Still create the repo file structure even if empty
    if let Some(parent) = file.repo_path.parent() {
      fs_manager.create_dir_all(parent)?;
    }
    // Create empty file if it doesn't exist (using copy from empty source)
    if !file.repo_path.exists() {
      fs_manager.copy(source, &file.repo_path)?;
    }
  } else {
    if let Some(parent) = file.repo_path.parent() {
      fs_manager.create_dir_all(parent)?;
    }

    if source.is_dir() {
      fs_manager.copy_dir_all(source, &file.repo_path)?;
    } else {
      fs_manager.copy(source, &file.repo_path)?;
    }
  }
  Ok(())
}

/// Compute the link target.
///
/// # Returns
///
/// Returns the link target.
fn compute_link_target(
  file: &TrackedFile,
  resolution: SymlinkResolution,
) -> Result<std::path::PathBuf> {
  Ok(match resolution {
    SymlinkResolution::Auto => {
      // Try relative, fall back to absolute
      file.dest_path.parent().map_or_else(
        || file.repo_path.clone(),
        |parent| {
          pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
        },
      )
    }
    SymlinkResolution::Relative => {
      let parent = file.dest_path.parent().ok_or_else(|| {
        DotfilesError::Path("Cannot determine parent for destination path".to_string())
      })?;
      pathdiff::diff_paths(&file.repo_path, parent)
        .ok_or_else(|| DotfilesError::Path("Cannot create relative symlink".to_string()))?
    }
    SymlinkResolution::Absolute => file.repo_path.clone(),
    SymlinkResolution::Follow => {
      // For migration, just use relative
      file.dest_path.parent().map_or_else(
        || file.repo_path.clone(),
        |parent| {
          pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
        },
      )
    }
    SymlinkResolution::Replace => {
      // Replace mode doesn't use symlinks, but we'll still compute a target
      // (This shouldn't be called in Replace mode, but handle it anyway)
      file.dest_path.parent().map_or_else(
        || file.repo_path.clone(),
        |parent| {
          pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
        },
      )
    }
  })
}
