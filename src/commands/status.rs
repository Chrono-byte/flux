use crate::config::Config;
use crate::types::TrackedFile;
use crate::utils::error::Result;
use crate::utils::path_utils::{
    files_differ, find_symlinked_parent, normalize_path, resolve_symlink_target,
    symlink_points_to_correct_target,
};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;

/// Status of a tracked file.
#[derive(Debug, Clone)]
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
pub struct StatusReport {
    /// The tracked file
    pub file: TrackedFile,
    /// Current status of the file
    pub status: FileStatus,
    /// Human-readable status message
    pub message: String,
}

/// Check the status of all tracked files.
///
/// # Errors
///
/// Returns an error if there are issues reading files or the configuration.
pub fn check_status(config: &Config, profile: Option<&str>) -> Result<Vec<StatusReport>> {
    let tracked_files = config.get_tracked_files(profile)?;
    let mut reports = Vec::new();

    for file in tracked_files {
        let status = check_file_status(&file)?;
        let message = status_message(&file, &status);
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
fn status_message(file: &TrackedFile, status: &FileStatus) -> String {
    match status {
        FileStatus::Synced => format!("✓ {}", file.dest_path.display()),
        FileStatus::MissingSymlink => format!("⊘ Missing: {}", file.dest_path.display()),
        FileStatus::BrokenSymlink => format!("⚠ Broken symlink: {}", file.dest_path.display()),
        FileStatus::OutOfSync => format!("↻ Out of sync: {}", file.dest_path.display()),
        FileStatus::MissingRepo => format!("✗ Missing repo file: {}", file.repo_path.display()),
    }
}

/// Filter status reports to only show conflicted files (git merge conflicts).
///
/// # Errors
///
/// Returns an error if there are issues reading the git repository or configuration.
pub fn check_conflicts(config: &Config, profile: Option<&str>) -> Result<Vec<StatusReport>> {
    use git2::Repository;

    let repo_path = config.get_repo_path();
    let Ok(repo) = Repository::open(&repo_path) else {
        // Not a git repo, no conflicts possible
        return Ok(Vec::new());
    };

    // Get git status entries
    let mut status_options = git2::StatusOptions::new();
    status_options.include_ignored(false);
    status_options.include_untracked(false);
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

/// Display the status of the tracked files.
///
/// # Parameters
///
/// * `reports`: The status reports to display.
///
/// # Returns
///
/// Returns the status of the tracked files.
pub fn display_status(reports: &[StatusReport]) {
    if reports.is_empty() {
        println!("{}", "No tracked files found.".yellow());
        return;
    }

    let synced_count = reports
        .iter()
        .filter(|result| matches!(result.status, FileStatus::Synced))
        .count();
    let issues_count = reports.len() - synced_count;

    // Group by tool
    let mut by_tool: HashMap<String, Vec<&StatusReport>> = HashMap::new();
    for report in reports {
        by_tool
            .entry(report.file.tool.clone())
            .or_default()
            .push(report);
    }

    println!("\n{}", "Dotfiles Status:".bold().cyan());
    println!("{}", "=".repeat(60).cyan());

    let mut tool_names: Vec<_> = by_tool.keys().collect();
    tool_names.sort();

    let empty_vec = Vec::new();
    for tool in tool_names {
        let tool_reports = by_tool.get(tool).unwrap_or(&empty_vec);
        println!(
            "\n{} {} ({} file(s))",
            "Tool:".bold(),
            tool.cyan(),
            tool_reports.len()
        );

        for report in tool_reports {
            let icon = match report.status {
                FileStatus::Synced => "✓".green(),
                FileStatus::MissingSymlink => "⊘".yellow(),
                FileStatus::BrokenSymlink => "⚠".yellow(),
                FileStatus::OutOfSync => "↻".yellow(),
                FileStatus::MissingRepo => "✗".red(),
            };

            let profile_info = if let Some(profile) = &report.file.profile {
                format!(" [profile: {profile}]").yellow()
            } else {
                "".normal()
            };

            println!("  {} {}{}", icon, report.message, profile_info);
        }
    }

    println!("\n{}", "=".repeat(60).cyan());
    println!(
        "{} {} synced, {} need attention",
        "Summary:".bold(),
        synced_count.to_string().green(),
        issues_count.to_string().yellow()
    );
}
