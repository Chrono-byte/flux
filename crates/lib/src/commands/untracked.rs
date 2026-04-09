use crate::config::Config;
use crate::types::TrackedFile;
use crate::utils::error::Result;
use crate::utils::path_utils::{
  files_differ, resolve_symlink_target, symlink_points_to_correct_target,
};
use std::fs;

/// A discrepancy found in a tracked file.
#[derive(Debug)]
pub struct Discrepancy {
  /// The tracked file with the issue
  pub file: TrackedFile,
  /// Type of issue detected
  pub issue: IssueType,
  /// Human-readable description of the issue
  pub message: String,
}

/// Type of issue detected in a tracked file.
#[derive(Debug, Clone, Copy)]
pub enum IssueType {
  /// File doesn't exist at expected location
  Missing,
  /// File exists but is not a symlink (should be symlink)
  NotSymlink,
  /// File is a symlink but points to wrong location
  WrongTarget,
  /// File exists but content differs from repo
  ContentDiffers,
  /// Repo file doesn't exist
  MissingRepo,
  /// Symlink is broken (target doesn't exist)
  BrokenSymlink,
}

/// Find discrepancies between tracked files and actual filesystem state.
///
/// # Errors
///
/// Returns an error if there are issues reading files or the configuration.
pub fn find_discrepancies(
  config: &Config,
  profile: Option<&str>,
) -> Result<Vec<Discrepancy>> {
  let tracked_files = config.get_tracked_files(profile)?;
  let mut discrepancies = Vec::new();

  for file in tracked_files {
    if let Some(discrepancy) = check_file_discrepancy(&file)? {
      discrepancies.push(discrepancy);
    }
  }

  // Sort by tool, then by path
  discrepancies.sort_by(|first, second| {
    first
      .file
      .tool
      .cmp(&second.file.tool)
      .then_with(|| first.file.dest_path.cmp(&second.file.dest_path))
  });

  Ok(discrepancies)
}

/// Check if a file has a discrepancy with the tracked file.
///
/// # Errors
///
/// Returns an error if there are issues reading files or the configuration.
fn check_file_discrepancy(file: &TrackedFile) -> Result<Option<Discrepancy>> {
  // First check: repo file exists
  if !file.repo_path.exists() {
    return Ok(Some(Discrepancy {
      file: file.clone(),
      issue: IssueType::MissingRepo,
      message: format!("Repo file does not exist: {}", file.repo_path.display()),
    }));
  }

  // Second check: destination file exists
  if !file.dest_path.exists() {
    return Ok(Some(Discrepancy {
      file: file.clone(),
      issue: IssueType::Missing,
      message: format!("Expected file does not exist: {}", file.dest_path.display()),
    }));
  }

  // Third check: is it a symlink?
  let is_symlink = file.dest_path.is_symlink();

  if is_symlink {
    // Check symlink target
    match fs::read_link(&file.dest_path) {
      Ok(link_target) => {
        let resolved_target = resolve_symlink_target(&file.dest_path, &link_target);

        // Check if symlink is broken (target doesn't exist)
        if !resolved_target.exists() {
          return Ok(Some(Discrepancy {
            file: file.clone(),
            issue: IssueType::BrokenSymlink,
            message: format!(
              "Symlink is broken (target doesn't exist): {} -> {}",
              file.dest_path.display(),
              link_target.display()
            ),
          }));
        }

        // Check if symlink points to correct location
        if !symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path) {
          return Ok(Some(Discrepancy {
            file: file.clone(),
            issue: IssueType::WrongTarget,
            message: format!(
              "Symlink points to wrong location: {} -> {} (expected: {})",
              file.dest_path.display(),
              link_target.display(),
              file.repo_path.display()
            ),
          }));
        }

        // For symlinks, we consider them correct if they point to the right place
        Ok(None)
      }
      Err(error) => {
        // Can't read symlink - treat as broken
        Ok(Some(Discrepancy {
          file: file.clone(),
          issue: IssueType::BrokenSymlink,
          message: format!(
            "Cannot read symlink: {} ({})",
            file.dest_path.display(),
            error
          ),
        }))
      }
    }
  } else {
    // Not a symlink - check if content differs
    if files_differ(&file.repo_path, &file.dest_path)? {
      Ok(Some(Discrepancy {
        file: file.clone(),
        issue: IssueType::ContentDiffers,
        message: format!(
          "File content differs from repo: {}",
          file.dest_path.display()
        ),
      }))
    } else {
      // File exists, content matches, but it's not a symlink
      // This might be intentional (e.g., if symlink_resolution is "replace")
      // But typically we expect symlinks, so we'll flag it
      Ok(Some(Discrepancy {
        file: file.clone(),
        issue: IssueType::NotSymlink,
        message: format!(
          "File exists but is not a symlink (expected symlink to {}): {}",
          file.repo_path.display(),
          file.dest_path.display()
        ),
      }))
    }
  }
}
