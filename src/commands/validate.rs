use crate::config::Config;
use crate::types::TrackedFile;
use crate::utils::error::Result;
use colored::Colorize;
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
pub struct ValidationReport {
    /// All issues found during validation
    pub issues: Vec<ValidationIssue>,
    /// Whether the configuration is valid (no issues)
    pub is_valid: bool,
}

pub fn validate_config(config: &Config) -> Result<ValidationReport> {
    let mut issues = Vec::new();
    let repo_path = config.get_repo_path()?;

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
            let normalized_repo = if file_entry.repo.starts_with(&format!("{}/", tool)) {
                // Remove tool name prefix (e.g., "cursor/settings.json" -> "settings.json")
                file_entry
                    .repo
                    .strip_prefix(&format!("{}/", tool))
                    .unwrap_or(&file_entry.repo)
                    .to_string()
            } else {
                file_entry.repo.clone()
            };
            tracked_files.insert(normalized_repo);
        }

        // Find orphaned files
        for repo_file in repo_files {
            if !tracked_files.contains(&repo_file) {
                issues.push(ValidationIssue::OrphanedEntry(tool.clone(), repo_file));
            }
        }
    }

    Ok(())
}

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
                files.insert(relative.to_string_lossy().to_string());
            }
        } else if path.is_dir() {
            collect_files(base, &path, files)?;
        }
    }

    Ok(())
}

pub fn display_validation(report: &ValidationReport) {
    if report.is_valid {
        println!("{}", "✓ Configuration is valid!".green().bold());
        return;
    }

    println!("\n{}", "Validation Issues:".bold().red());
    println!("{}", "=".repeat(60).red());

    for issue in &report.issues {
        match issue {
            ValidationIssue::MissingRepoFile(file) => {
                println!(
                    "{} Missing repo file: {}",
                    "✗".red(),
                    file.repo_path.display()
                );
            }
            ValidationIssue::InvalidSymlink(file) => {
                println!(
                    "{} Invalid symlink: {}",
                    "⚠".yellow(),
                    file.dest_path.display()
                );
            }
            ValidationIssue::OrphanedEntry(tool, file) => {
                println!("{} Orphaned file in {}: {}", "⊘".yellow(), tool, file);
            }
            ValidationIssue::MissingProfileDir(profile) => {
                println!("{} Missing profile directory: {}", "⚠".yellow(), profile);
            }
            ValidationIssue::InvalidConfig(msg) => {
                println!("{} {}", "✗".red(), msg);
            }
        }
    }

    println!("{}", "=".repeat(60).red());
    println!(
        "{} Found {} issue(s)",
        "Summary:".bold(),
        report.issues.len().to_string().red()
    );
}

/// Normalize a path by canonicalizing it, falling back to the path itself if canonicalization fails
fn normalize_path(path: &Path) -> PathBuf {
    // Try to canonicalize, but fall back to the path itself if it fails
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
