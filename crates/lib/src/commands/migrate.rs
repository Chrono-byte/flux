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

enum MigrationResult {
  Migrated,
  Skipped,
}

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
    IssueType::MissingRepo => Ok(MigrationResult::Skipped),
  }
}

fn migrate_missing_file(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
  if !file.repo_path.exists() {
    return Ok(MigrationResult::Skipped);
  }

  if let Some(parent) = file.dest_path.parent() {
    fs_manager.create_dir_all(parent)?;
  }

  let link_target = compute_link_target(file, resolution)?;
  fs_manager.symlink(&link_target, &file.dest_path)?;
  Ok(MigrationResult::Migrated)
}

fn migrate_not_symlink_or_differs(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
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

  fs_manager.remove_file(&file.dest_path)?;

  let link_target = compute_link_target(file, resolution)?;
  fs_manager.symlink(&link_target, &file.dest_path)?;
  Ok(MigrationResult::Migrated)
}

fn migrate_wrong_target_or_broken(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<MigrationResult> {
  let source_to_copy = find_source_to_copy(file)?;

  if let Some(source) = &source_to_copy {
    copy_source_to_repo(source, file, fs_manager)?;
  }

  fs_manager.remove_file(&file.dest_path)?;

  let link_target = compute_link_target(file, resolution)?;
  fs_manager.symlink(&link_target, &file.dest_path)?;
  Ok(MigrationResult::Migrated)
}

fn find_source_to_copy(file: &TrackedFile) -> Result<Option<std::path::PathBuf>> {
  if file.dest_path.is_symlink() {
    let link_target = fs::read_link(&file.dest_path)?;
    Ok({
      let resolved_target = if link_target.is_absolute() {
        link_target
      } else {
        match file.dest_path.parent() {
          Some(parent) => parent.join(&link_target),
          None => link_target,
        }
      };

      if resolved_target.exists() {
        let is_empty = if resolved_target.is_file() {
          fs::metadata(&resolved_target)?.len() == 0
        } else {
          false
        };

        if is_empty {
          if file.dest_path.exists() {
            match fs::read(&file.dest_path) {
              Ok(content) if !content.is_empty() => Some(file.dest_path.clone()),
              _ => Some(resolved_target),
            }
          } else {
            Some(resolved_target)
          }
        } else {
          Some(resolved_target)
        }
      } else {
        if file.dest_path.exists() {
          match fs::read(&file.dest_path) {
            Ok(content) if !content.is_empty() => Some(file.dest_path.clone()),
            _ => None,
          }
        } else {
          None
        }
      }
    })
  } else if file.dest_path.exists() {
    Ok(Some(file.dest_path.clone()))
  } else {
    Ok(None)
  }
}

fn copy_source_to_repo(
  source: &std::path::Path,
  file: &TrackedFile,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  let source_is_empty = if source.is_file() {
    fs::metadata(source)?.len() == 0
  } else {
    false
  };

  if source_is_empty {
    if let Some(parent) = file.repo_path.parent() {
      fs_manager.create_dir_all(parent)?;
    }
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

fn compute_link_target(
  file: &TrackedFile,
  resolution: SymlinkResolution,
) -> Result<std::path::PathBuf> {
  Ok(match resolution {
    SymlinkResolution::Auto => file.dest_path.parent().map_or_else(
      || file.repo_path.clone(),
      |parent| {
        pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
      },
    ),
    SymlinkResolution::Relative => {
      let parent = file.dest_path.parent().ok_or_else(|| {
        DotfilesError::Path("Cannot determine parent for destination path".to_string())
      })?;
      pathdiff::diff_paths(&file.repo_path, parent)
        .ok_or_else(|| DotfilesError::Path("Cannot create relative symlink".to_string()))?
    }
    SymlinkResolution::Absolute => file.repo_path.clone(),
    SymlinkResolution::Follow => file.dest_path.parent().map_or_else(
      || file.repo_path.clone(),
      |parent| {
        pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
      },
    ),
    SymlinkResolution::Replace => file.dest_path.parent().map_or_else(
      || file.repo_path.clone(),
      |parent| {
        pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
      },
    ),
  })
}
