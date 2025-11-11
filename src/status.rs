use crate::config::Config;
use crate::error::Result;
use crate::file_manager::is_file_locked;
use crate::types::TrackedFile;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum FileStatus {
    Synced,
    MissingSymlink,
    BrokenSymlink,
    OutOfSync,
    Locked,
    MissingRepo,
}

pub struct StatusReport {
    pub file: TrackedFile,
    pub status: FileStatus,
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

    // Check if file is locked
    if is_file_locked(&file.dest_path) || is_file_locked(&file.repo_path) {
        return Ok(FileStatus::Locked);
    }

    // Check if destination exists
    if !file.dest_path.exists() {
        return Ok(FileStatus::MissingSymlink);
    }

    // Check if it's a symlink
    if let Ok(link_target) = fs::read_link(&file.dest_path) {
        // Check if symlink is broken
        if !link_target.exists() {
            return Ok(FileStatus::BrokenSymlink);
        }

        // Check if symlink points to correct location
        // Resolve relative symlink targets to absolute paths for comparison
        let resolved_target = if link_target.is_absolute() {
            link_target
        } else {
            file.dest_path
                .parent()
                .map(|p| p.join(&link_target))
                .unwrap_or(link_target)
        };

        // Normalize both paths before comparing
        let normalized_target = normalize_path(&resolved_target);
        let normalized_repo = normalize_path(&file.repo_path);

        if normalized_target != normalized_repo {
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

fn files_differ(path1: &Path, path2: &Path) -> Result<bool> {
    if !path1.exists() || !path2.exists() {
        return Ok(true);
    }

    if path1.is_dir() || path2.is_dir() {
        // For directories, we consider them different if one is dir and other isn't
        return Ok(path1.is_dir() != path2.is_dir());
    }

    let content1 = fs::read(path1)?;
    let content2 = fs::read(path2)?;

    Ok(content1 != content2)
}

fn status_message(file: &TrackedFile, status: &FileStatus) -> String {
    match status {
        FileStatus::Synced => format!("✓ {}", file.dest_path.display()),
        FileStatus::MissingSymlink => format!("⊘ Missing: {}", file.dest_path.display()),
        FileStatus::BrokenSymlink => format!("⚠ Broken symlink: {}", file.dest_path.display()),
        FileStatus::OutOfSync => format!("↻ Out of sync: {}", file.dest_path.display()),
        FileStatus::Locked => format!("🔒 Locked: {}", file.dest_path.display()),
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
                FileStatus::Locked => "🔒".yellow(),
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

/// Normalize a path by canonicalizing it, falling back to the path itself if canonicalization fails
fn normalize_path(path: &Path) -> PathBuf {
    // Try to canonicalize, but fall back to the path itself if it fails
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
