use crate::config::Config;
use crate::types::TrackedFile;
use crate::utils::error::Result;
use crate::utils::path_utils::{
  files_differ, find_symlinked_parent, normalize_path, resolve_symlink_target,
  symlink_points_to_correct_target,
};
use colored::Colorize;
use serde::Serialize;
use std::fs;
use std::time::SystemTime;

/// Status of a tracked file.
#[derive(Debug, Clone, Copy)]
pub enum FileStatus {
  /// File is correctly synced
  Synced,
  /// Symlink is missing at destination
  MissingSymlink,
  /// Symlink exists but target is broken
  BrokenSymlink,
  /// File is out of sync with repository
  OutOfSync,
  /// Repository file is missing
  MissingRepo,
}

/// Status report for a tracked file.
#[derive(Debug, Clone)]
pub struct StatusReport {
  /// The tracked file
  pub file: TrackedFile,
  /// Current status of the file
  pub status: FileStatus,
  /// Human-readable status message
  pub message: String,
}

/// JSON-serializable status report for scripting.
#[derive(Debug, Serialize)]
pub struct JsonStatusReport {
  /// Tool name
  pub tool: String,
  /// Repository path
  pub repo_path: String,
  /// Destination path
  pub dest_path: String,
  /// Profile name (if any)
  pub profile: Option<String>,
  /// Status as string
  pub status: String,
  /// Human-readable message
  pub message: String,
  /// File size in bytes (if available)
  pub size: Option<u64>,
  /// Last modified time (if available)
  pub modified: Option<String>,
}

impl From<&StatusReport> for JsonStatusReport {
  fn from(report: &StatusReport) -> Self {
    // Get file size if available
    let size = report
      .file
      .repo_path
      .metadata()
      .map(|metadata| metadata.len())
      .ok();

    // Get last modified time as seconds since UNIX_EPOCH (as string), if available
    let modified = report
      .file
      .repo_path
      .metadata()
      .ok()
      .and_then(|metadata| metadata.modified().ok())
      .and_then(|mtime| mtime.duration_since(SystemTime::UNIX_EPOCH).ok())
      .map(|duration| duration.as_secs().to_string());

    Self {
      tool: report.file.tool.clone(),
      repo_path: report.file.repo_path.display().to_string(),
      dest_path: report.file.dest_path.display().to_string(),
      profile: report.file.profile.clone(),
      status: format!("{:?}", report.status),
      message: report.message.clone(),
      size,
      modified,
    }
  }
}

/// Check the status of all tracked files.
///
/// # Errors
///
/// Returns an error if there are issues reading files or the configuration.
pub fn check_status(
  config: &Config,
  profile: Option<&str>,
) -> Result<Vec<StatusReport>> {
  let tracked_files = config.get_tracked_files(profile)?;
  let mut reports = Vec::new();

  for file in tracked_files {
    let status = check_file_status(&file)?;
    let message = status_message(&file, status);
    reports.push(StatusReport {
      file,
      status,
      message,
    });
  }

  Ok(reports)
}

/// Check the status of a tracked file.
///
/// # Errors
///
/// Returns an error if there are issues reading files or the configuration.
fn check_file_status(file: &TrackedFile) -> Result<FileStatus> {
  // Check if repo file exists
  if !file.repo_path.exists() {
    return Ok(FileStatus::MissingRepo);
  }

  // Check if destination exists
  if !file.dest_path.exists() {
    return Ok(FileStatus::MissingSymlink);
  }

  // Check if it's a symlink
  if let Ok(link_target) = fs::read_link(&file.dest_path) {
    let resolved_target = resolve_symlink_target(&file.dest_path, &link_target);

    // Check if symlink is broken
    if !resolved_target.exists() {
      return Ok(FileStatus::BrokenSymlink);
    }

    // Check if symlink points to correct location
    if !symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path) {
      return Ok(FileStatus::OutOfSync);
    }

    // Check if files differ (for non-symlink cases or if symlink resolution is "replace")
    if files_differ(&file.repo_path, &file.dest_path)? {
      return Ok(FileStatus::OutOfSync);
    }

    Ok(FileStatus::Synced)
  } else {
    // Destination exists but is not a symlink
    // Check if a parent directory is symlinked - if so, the file might be effectively synced
    // via the directory symlink
    if let Some((parent_dir, resolved_parent)) = find_symlinked_parent(&file.dest_path) {
      // Get the relative path from the symlinked parent to the file
      if let Ok(relative) = file.dest_path.strip_prefix(parent_dir) {
        let resolved_file_path = resolved_parent.join(relative);
        let normalized_resolved = normalize_path(&resolved_file_path);
        let normalized_repo = normalize_path(&file.repo_path);

        // If the resolved path through the symlinked parent matches the repo path,
        // and the file content matches, it's effectively synced
        if normalized_resolved == normalized_repo
          && !files_differ(&file.repo_path, &file.dest_path)?
        {
          // Files match and are effectively synced via directory symlink
          // Return OutOfSync to indicate it should be converted to individual symlink
          // for better management
          return Ok(FileStatus::OutOfSync);
        }
      }
    }

    // Destination exists but is not a symlink and content differs
    Ok(FileStatus::OutOfSync)
  }
}

/// Get the status message for a tracked file.
///
/// # Returns
///
/// Returns the status message for a tracked file.
fn status_message(
  file: &TrackedFile,
  status: FileStatus,
) -> String {
  match status {
    FileStatus::Synced => format!("✓ {}", file.dest_path.display()),
    FileStatus::MissingSymlink => {
      format!("⊘ Missing: {}", file.dest_path.display())
    }
    FileStatus::BrokenSymlink => {
      format!("⚠ Broken symlink: {}", file.dest_path.display())
    }
    FileStatus::OutOfSync => {
      format!("↻ Out of sync: {}", file.dest_path.display())
    }
    FileStatus::MissingRepo => {
      format!("✗ Missing repo file: {}", file.repo_path.display())
    }
  }
}

/// Filter status reports to only show conflicted files (git merge conflicts).
///
/// # Errors
///
/// Returns an error if there are issues reading the git repository or configuration.
pub fn check_conflicts(
  config: &Config,
  profile: Option<&str>,
) -> Result<Vec<StatusReport>> {
  use git2::Repository;

  let repo_path = config.get_repo_path();
  let Ok(repo) = Repository::open(&repo_path) else {
    // Not a git repo, no conflicts possible
    return Ok(Vec::new());
  };

  // Get git status entries
  let mut status_options = git2::StatusOptions::new();
  let _ = status_options.include_ignored(false);
  let _ = status_options.include_untracked(false);
  let statuses = repo.statuses(Some(&mut status_options))?;

  let tracked_files = config.get_tracked_files(profile)?;
  let mut conflicted = Vec::new();

  for file in tracked_files {
    // Check if this file is in a conflicted state in git
    let repo_relative = match file.repo_path.strip_prefix(&repo_path) {
      Ok(rel) => rel.to_string_lossy().to_string(),
      Err(_) => continue,
    };

    // Check git status for this file
    for entry in statuses.iter() {
      let entry_path = match entry.path() {
        Some(path) => path.to_string(),
        None => continue,
      };
      if entry_path == repo_relative || entry_path.ends_with(&repo_relative) {
        let status_flags = entry.status();
        // Check for conflict markers (both modified, deleted by them, etc.)
        if status_flags.contains(git2::Status::CONFLICTED) {
          let status = check_file_status(&file)?;
          conflicted.push(StatusReport {
            file,
            status,
            message: format!("⚠ Git conflict: {repo_relative}"),
          });
          break;
        }
      }
    }
  }

  Ok(conflicted)
}

/// Display the status of the tracked files with git-like formatting.
///
/// # Parameters
///
/// * `reports`: The status reports to display.
pub fn display_status(reports: &[StatusReport]) {
  if reports.is_empty() {
    println!("{}", "No tracked files found.".yellow());
    return;
  }

  // Group reports by status type
  let mut synced: Vec<&StatusReport> = Vec::new();
  let mut missing_symlinks: Vec<&StatusReport> = Vec::new();
  let mut broken_symlinks: Vec<&StatusReport> = Vec::new();
  let mut out_of_sync: Vec<&StatusReport> = Vec::new();
  let mut missing_repo: Vec<&StatusReport> = Vec::new();

  for report in reports {
    match report.status {
      FileStatus::Synced => synced.push(report),
      FileStatus::MissingSymlink => missing_symlinks.push(report),
      FileStatus::BrokenSymlink => broken_symlinks.push(report),
      FileStatus::OutOfSync => out_of_sync.push(report),
      FileStatus::MissingRepo => missing_repo.push(report),
    }
  }

  // Sort each group by destination path for consistent output
  synced.sort_by_key(|report| &report.file.dest_path);
  missing_symlinks.sort_by_key(|report| &report.file.dest_path);
  broken_symlinks.sort_by_key(|report| &report.file.dest_path);
  out_of_sync.sort_by_key(|report| &report.file.dest_path);
  missing_repo.sort_by_key(|report| &report.file.repo_path);

  // Display issues sections (similar to git's "Changes not staged for commit")
  let has_issues = !missing_symlinks.is_empty()
    || !broken_symlinks.is_empty()
    || !out_of_sync.is_empty()
    || !missing_repo.is_empty();

  if has_issues {
    // Missing symlinks (similar to "Changes not staged for commit")
    if !missing_symlinks.is_empty() {
      println!("\n{}", "Missing symlinks:".yellow().bold());
      for report in &missing_symlinks {
        println!("  {}", format_file_path(&report.file.dest_path).yellow());
      }
    }

    // Broken symlinks
    if !broken_symlinks.is_empty() {
      println!("\n{}", "Broken symlinks:".yellow().bold());
      for report in &broken_symlinks {
        println!("  {}", format_file_path(&report.file.dest_path).yellow());
      }
    }

    // Out of sync
    if !out_of_sync.is_empty() {
      println!("\n{}", "Out of sync:".yellow().bold());
      for report in &out_of_sync {
        println!("  {}", format_file_path(&report.file.dest_path).yellow());
      }
    }

    // Missing repo files
    if !missing_repo.is_empty() {
      println!("\n{}", "Missing repo files:".red().bold());
      for report in &missing_repo {
        println!("  {}", format_file_path(&report.file.repo_path).red());
      }
    }
  }

  // Summary (similar to git status summary)
  let total = reports.len();
  let synced_count = synced.len();
  let issues_count = total - synced_count;

  if issues_count == 0 {
    println!("\n{}", "nothing to sync, working tree clean".green());
  } else {
    println!(
      "\n{} {} file(s) synced, {} file(s) need attention",
      "Summary:".bold(),
      synced_count.to_string().green(),
      issues_count.to_string().yellow()
    );
  }
}

/// Format a file path for display, showing relative path when possible.
fn format_file_path(path: &std::path::Path) -> String {
  // Try to show relative path from current directory, fallback to full path
  std::env::current_dir()
    .ok()
    .and_then(|cwd| path.strip_prefix(&cwd).ok())
    .map_or_else(
      || path.display().to_string(),
      |rel| rel.display().to_string(),
    )
}
