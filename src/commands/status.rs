use crate::config::Config;
use crate::types::TrackedFile;
use crate::utils::error::Result;
use crate::utils::path_utils::{files_differ, resolve_symlink_target, symlink_points_to_correct_target};
use colored::Colorize;
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
        // Destination exists but is not a symlink - this is always out of sync
        Ok(FileStatus::OutOfSync)
    }
}


fn status_message(file: &TrackedFile, status: &FileStatus) -> String {
    match status {
        FileStatus::Synced => format!("✓ {}", file.dest_path.display()),
        FileStatus::MissingSymlink => format!("⊘ Missing: {}", file.dest_path.display()),
        FileStatus::BrokenSymlink => format!("⚠ Broken symlink: {}", file.dest_path.display()),
        FileStatus::OutOfSync => format!("↻ Out of sync: {}", file.dest_path.display()),
        FileStatus::MissingRepo => format!("✗ Missing repo file: {}", file.repo_path.display()),
    }
}

pub fn display_status(reports: &[StatusReport]) {
    if reports.is_empty() {
        println!("{}", "No tracked files found.".yellow());
        return;
    }

    let synced_count = reports
        .iter()
        .filter(|r| matches!(r.status, FileStatus::Synced))
        .count();
    let issues_count = reports.len() - synced_count;

    // Group by tool
    use std::collections::HashMap;
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

    for tool in tool_names {
        let tool_reports = &by_tool[tool];
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
                format!(" [profile: {}]", profile).yellow()
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

