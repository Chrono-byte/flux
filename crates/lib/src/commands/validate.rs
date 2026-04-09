use crate::config::Config;
use crate::types::TrackedFile;
use crate::utils::error::Result;
use crate::utils::path_utils::symlink_points_to_correct_target;
use std::fs;
use std::path::{Path, PathBuf};

/// A validation issue found in the configuration.
#[derive(Debug, Clone)]
pub enum ValidationIssue {
  /// Repository file is missing
  MissingRepoFile(TrackedFile),
  /// Symlink is invalid or broken
  InvalidSymlink(TrackedFile),
  /// File exists in repo but not tracked in config
  OrphanedEntry(String, String), // tool, file
  /// Profile directory is missing
  MissingProfileDir(String),
  /// Configuration has invalid values
  InvalidConfig(String),
}

/// Report from configuration validation.
#[derive(Debug)]
pub struct ValidationReport {
  /// All issues found during validation
  pub issues: Vec<ValidationIssue>,
  /// Whether the configuration is valid (no issues)
  pub is_valid: bool,
}

/// Validate the configuration and return a report of any issues.
///
/// # Errors
///
/// Returns an error if there are issues reading files or the configuration.
pub fn validate_config(config: &Config) -> Result<ValidationReport> {
  let mut issues = Vec::new();
  let repo_path = config.get_repo_path();

  // Check if repo exists
  if !repo_path.exists() {
    issues.push(ValidationIssue::InvalidConfig(format!(
      "Repository path does not exist: {}",
      repo_path.display()
    )));
  }

  // Validate all tracked files
  let tracked_files = config.get_tracked_files(None)?;

  for file in &tracked_files {
    // Check if repo file exists
    if !file.repo_path.exists() {
      issues.push(ValidationIssue::MissingRepoFile(file.clone()));
      continue;
    }

    // Check if destination is a symlink
    if file.dest_path.exists() && file.dest_path.is_symlink() {
      if let Ok(link_target) = fs::read_link(&file.dest_path) {
        if !symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path) {
          issues.push(ValidationIssue::InvalidSymlink(file.clone()));
        }
      } else {
        // It's a symlink but we can't read it (broken)
        issues.push(ValidationIssue::InvalidSymlink(file.clone()));
      }
    }
  }

  // Check for orphaned entries (files in repo but not in config)
  if repo_path.exists() {
    check_orphaned_entries(&repo_path, config, &mut issues)?;
  }

  // Validate profiles
  let profiles_dir = repo_path.join("profiles");
  if profiles_dir.exists() {
    for entry in fs::read_dir(&profiles_dir)? {
      let entry = entry?;
      if entry.path().is_dir() {
        let _profile_name = entry.file_name().to_string_lossy().to_string();
        // Profile directory exists, which is good
      }
    }
  }

  // Check if current profile directory exists
  let current_profile_dir = repo_path
    .join("profiles")
    .join(&config.general.current_profile);
  if config.general.current_profile != "default" && !current_profile_dir.exists() {
    issues.push(ValidationIssue::MissingProfileDir(
      config.general.current_profile.clone(),
    ));
  }

  Ok(ValidationReport {
    is_valid: issues.is_empty(),
    issues,
  })
}

/// Check for orphaned entries in the repository.
///
/// # Parameters
///
/// * `repo_path`: The path to the repository.
/// * `config`: The configuration.
/// * `issues`: The issues to check for.
///
/// # Returns
///
/// Returns the orphaned entries.
///
/// # Errors
///
/// Returns an error if there are issues reading files or the configuration.
fn check_orphaned_entries(
  repo_path: &Path,
  config: &Config,
  issues: &mut Vec<ValidationIssue>,
) -> Result<()> {
  // Check each tool directory
  for (tool, tool_config) in &config.tools {
    let tool_dir = repo_path.join(tool);

    if !tool_dir.exists() {
      continue;
    }

    // Get all files in tool directory
    let mut repo_files = std::collections::HashSet::new();
    collect_files(&tool_dir, &tool_dir, &mut repo_files)?;

    // Get files tracked in config
    // Normalize repo paths: remove tool name prefix if present
    let mut tracked_files = std::collections::HashSet::new();
    for file_entry in &tool_config.files {
      let normalized_repo = normalize_repo_path(tool, &file_entry.repo);
      let _ = tracked_files.insert(normalized_repo);
    }

    // Collect folder coverage data
    let mut folder_paths = Vec::new();
    let mut root_folder_covered = false;
    for folder_entry in &tool_config.folders {
      let normalized_folder = normalize_repo_path(tool, &folder_entry.repo);
      if normalized_folder.is_empty() {
        root_folder_covered = true;
        break;
      }
      folder_paths.push(PathBuf::from(normalized_folder));
    }

    if root_folder_covered {
      // Entire tool directory is covered by a folder entry, so skip orphan checks.
      continue;
    }

    // Find orphaned files
    for repo_file in repo_files {
      let repo_path = Path::new(&repo_file);

      if folder_paths
        .iter()
        .any(|folder_path| repo_path.starts_with(folder_path))
      {
        continue;
      }

      if !tracked_files.contains(&repo_file) {
        issues.push(ValidationIssue::OrphanedEntry(tool.clone(), repo_file));
      }
    }
  }

  Ok(())
}

/// Normalize a repo path relative to its tool directory.
fn normalize_repo_path(
  tool: &str,
  repo: &str,
) -> String {
  let repo = repo.strip_prefix("./").unwrap_or(repo);
  let repo = repo.trim_end_matches('/');

  if repo.is_empty() || repo == "." || repo == tool {
    return String::new();
  }

  if let Some(stripped) = repo.strip_prefix(&format!("{tool}/")) {
    return stripped.to_string();
  }

  repo.to_string()
}

/// Collect files from a directory.
///
/// # Parameters
///
/// * `base`: The base path.
/// * `dir`: The directory to collect files from.
/// * `files`: The files to collect.
///
/// # Returns
///
/// Returns the collected files.
fn collect_files(
  base: &Path,
  dir: &Path,
  files: &mut std::collections::HashSet<String>,
) -> Result<()> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();

    if path.is_file() {
      if let Ok(relative) = path.strip_prefix(base) {
        let _ = files.insert(relative.to_string_lossy().to_string());
      }
    } else if path.is_dir() {
      collect_files(base, &path, files)?;
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;
  use std::path::Path;
  use tempfile::tempdir;

  #[test]
  fn folder_entries_cover_tool_root() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let repo_dir = tempdir()?;
    let tool_dir = repo_dir.path().join("nvim");
    fs::create_dir_all(&tool_dir)?;

    fs::write(tool_dir.join("LICENSE"), "license")?;
    fs::write(tool_dir.join("init.lua"), "init")?;

    let mut config = Config::default();
    config.general.repo_path = repo_dir.path().to_string_lossy().to_string();
    config.add_folder_to_tool("nvim", "nvim", Path::new(".config/nvim"), None);

    let mut issues = Vec::new();
    check_orphaned_entries(repo_dir.path(), &config, &mut issues)?;

    assert!(
      issues.is_empty(),
      "Folder entry should cover entire tool directory"
    );

    Ok(())
  }

  #[test]
  fn normalize_repo_path_handles_special_values() {
    assert_eq!(normalize_repo_path("nvim", "nvim"), "");
    assert_eq!(normalize_repo_path("nvim", "./nvim"), "");
    assert_eq!(normalize_repo_path("nvim", "nvim/"), "");
    assert_eq!(normalize_repo_path("nvim", "./"), "");
    assert_eq!(normalize_repo_path("nvim", "."), "");
    assert_eq!(normalize_repo_path("nvim", "nvim/lua"), "lua");
    assert_eq!(normalize_repo_path("nvim", "nvim/lua/"), "lua");
    assert_eq!(normalize_repo_path("nvim", "./lua"), "lua");
  }

  #[test]
  fn orphan_detection_reports_untracked_file() -> std::result::Result<(), Box<dyn std::error::Error>>
  {
    let repo_dir = tempdir()?;
    let tool_dir = repo_dir.path().join("nvim");
    fs::create_dir_all(&tool_dir)?;

    fs::write(tool_dir.join("tracked.txt"), "ok")?;
    fs::write(tool_dir.join("orphan.txt"), "oops")?;

    let mut config = Config::default();
    config.general.repo_path = repo_dir.path().to_string_lossy().to_string();
    config.add_file_to_tool(
      "nvim",
      "tracked.txt",
      Path::new(".config/nvim/tracked.txt"),
      None,
    );

    let mut issues = Vec::new();
    check_orphaned_entries(repo_dir.path(), &config, &mut issues)?;

    assert!(
      issues.iter().any(
        |issue| matches!(issue, ValidationIssue::OrphanedEntry(tool, file) if tool == "nvim" && file == "orphan.txt")
      ),
      "Expected orphan entries for files not configured"
    );

    Ok(())
  }

  #[test]
  fn folder_entry_only_covers_subpath() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let repo_dir = tempdir()?;
    let tool_dir = repo_dir.path().join("nvim/lua");
    fs::create_dir_all(&tool_dir)?;

    fs::write(tool_dir.join("plugin.lua"), "ok")?;
    fs::write(repo_dir.path().join("nvim/README.md"), "readme")?;

    let mut config = Config::default();
    config.general.repo_path = repo_dir.path().to_string_lossy().to_string();
    config.add_folder_to_tool("nvim", "nvim/lua", Path::new(".config/nvim/lua"), None);

    let mut issues = Vec::new();
    check_orphaned_entries(repo_dir.path(), &config, &mut issues)?;

    assert!(
      issues.iter().any(
        |issue| matches!(issue, ValidationIssue::OrphanedEntry(tool, file) if tool == "nvim" && file == "README.md")
      ),
      "README.md should still be reported even when lua/ folder is tracked"
    );

    Ok(())
  }
}
