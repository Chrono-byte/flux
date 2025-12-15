use crate::config::Config;
use crate::types::TrackedFile;
use crate::utils::error::Result;
use crate::utils::path_utils::symlink_points_to_correct_target;
use colored::Colorize;
use std::fs;

/// A health issue detected by the doctor command.
#[derive(Debug, Clone)]
pub enum HealthIssue {
    /// Broken symlink (target doesn't exist)
    BrokenSymlink {
        path: PathBuf,
        target: PathBuf,
        reason: String,
    },
    /// Missing repository file referenced in config
    MissingRepoFile { file: TrackedFile },
    /// Config drift: file content differs from repo
    ConfigDrift { file: TrackedFile, summary: String },
    /// Invalid profile or environment configuration
    InvalidProfile { profile: String, reason: String },
    /// Git remote is unreachable or invalid
    BadGitRemote {
        remote: String,
        url: String,
        reason: String,
    },
    /// Unsafe file permissions detected
    UnsafePermissions {
        path: PathBuf,
        current_mode: String,
        recommended_mode: String,
    },
}

use std::path::PathBuf;

/// Health report from doctor command.
pub struct HealthReport {
    pub issues: Vec<HealthIssue>,
}

impl HealthReport {
    pub fn is_healthy(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn by_category(&self) -> std::collections::HashMap<String, Vec<&HealthIssue>> {
        let mut by_cat = std::collections::HashMap::new();
        for issue in &self.issues {
            let cat = match issue {
                HealthIssue::BrokenSymlink { .. } => "Symlinks",
                HealthIssue::MissingRepoFile { .. } => "Missing Files",
                HealthIssue::ConfigDrift { .. } => "Config Drift",
                HealthIssue::InvalidProfile { .. } => "Profiles",
                HealthIssue::BadGitRemote { .. } => "Git Remotes",
                HealthIssue::UnsafePermissions { .. } => "Permissions",
            };
            by_cat
                .entry(cat.to_string())
                .or_insert_with(Vec::new)
                .push(issue);
        }
        by_cat
    }
}

/// Run all health checks and return a report.
pub fn run_health_checks(config: &Config) -> Result<HealthReport> {
    let mut issues = Vec::new();

    // Check symlink health
    check_symlinks(config, &mut issues)?;

    // Check missing repo files
    check_missing_repo_files(config, &mut issues)?;

    // Check config drift
    check_config_drift(config, &mut issues)?;

    // Check invalid profiles
    check_invalid_profiles(config, &mut issues)?;

    // Check git remotes
    check_git_remotes(config, &mut issues)?;

    // Check unsafe permissions
    check_unsafe_permissions(config, &mut issues)?;

    Ok(HealthReport { issues })
}

fn check_symlinks(config: &Config, issues: &mut Vec<HealthIssue>) -> Result<()> {
    let tracked_files = config.get_tracked_files(None)?;

    for file in tracked_files {
        if !file.dest_path.exists() {
            continue; // Not a symlink issue if it doesn't exist
        }

        if let Ok(link_target) = fs::read_link(&file.dest_path) {
            let resolved = file.dest_path.parent().map(|p| p.join(&link_target));
            if let Some(resolved_path) = resolved {
                if !resolved_path.exists() {
                    issues.push(HealthIssue::BrokenSymlink {
                        path: file.dest_path.clone(),
                        target: link_target.clone(),
                        reason: "Target file does not exist".to_string(),
                    });
                } else if !symlink_points_to_correct_target(
                    &file.dest_path,
                    &link_target,
                    &file.repo_path,
                ) {
                    issues.push(HealthIssue::BrokenSymlink {
                        path: file.dest_path.clone(),
                        target: link_target.clone(),
                        reason: "Symlink points to wrong location".to_string(),
                    });
                }
            }
        }
    }

    Ok(())
}

fn check_missing_repo_files(config: &Config, issues: &mut Vec<HealthIssue>) -> Result<()> {
    let tracked_files = config.get_tracked_files(None)?;

    for file in tracked_files {
        if !file.repo_path.exists() {
            issues.push(HealthIssue::MissingRepoFile { file });
        }
    }

    Ok(())
}

fn check_config_drift(config: &Config, issues: &mut Vec<HealthIssue>) -> Result<()> {
    use crate::utils::path_utils::files_differ;

    let tracked_files = config.get_tracked_files(None)?;

    for file in tracked_files {
        if !file.repo_path.exists() || !file.dest_path.exists() {
            continue;
        }

        // Skip if it's a correctly linked symlink
        if file.dest_path.is_symlink()
            && let Ok(link_target) = fs::read_link(&file.dest_path)
                && symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path)
                {
                    continue;
                }

        // Check if content differs
        if let Ok(differ) = files_differ(&file.repo_path, &file.dest_path)
            && differ {
                issues.push(HealthIssue::ConfigDrift {
                    file,
                    summary: "File content differs from repository version".to_string(),
                });
            }
    }

    Ok(())
}

fn check_invalid_profiles(config: &Config, issues: &mut Vec<HealthIssue>) -> Result<()> {
    let repo_path = config.get_repo_path()?;
    let profiles_dir = repo_path.join("profiles");

    // Check if current profile directory exists
    if config.general.current_profile != "default" {
        let profile_dir = profiles_dir.join(&config.general.current_profile);
        if !profile_dir.exists() {
            issues.push(HealthIssue::InvalidProfile {
                profile: config.general.current_profile.clone(),
                reason: "Profile directory does not exist".to_string(),
            });
        }
    }

    Ok(())
}

fn check_git_remotes(config: &Config, issues: &mut Vec<HealthIssue>) -> Result<()> {
    use git2::Repository;

    let repo_path = config.get_repo_path()?;

    // Try to open repository
    let repo = match Repository::open(&repo_path) {
        Ok(r) => r,
        Err(_) => {
            // Not a git repo, skip remote checks
            return Ok(());
        }
    };

    let remotes = repo.remotes()?;

    for name_opt in remotes.iter() {
        if let Some(name) = name_opt
            && let Ok(remote) = repo.find_remote(name)
                && let Some(url) = remote.url() {
                    // Basic validation: check if URL looks valid
                    if url.is_empty() {
                        issues.push(HealthIssue::BadGitRemote {
                            remote: name.to_string(),
                            url: url.to_string(),
                            reason: "Remote URL is empty".to_string(),
                        });
                    } else if !url.starts_with("http://")
                        && !url.starts_with("https://")
                        && !url.starts_with("git@")
                        && !url.starts_with("file://")
                    {
                        // Not a comprehensive check, but catches obviously malformed URLs
                        issues.push(HealthIssue::BadGitRemote {
                            remote: name.to_string(),
                            url: url.to_string(),
                            reason: "Remote URL format looks invalid".to_string(),
                        });
                    }
                }
    }

    Ok(())
}

fn check_unsafe_permissions(config: &Config, issues: &mut Vec<HealthIssue>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let tracked_files = config.get_tracked_files(None)?;
    let home = dirs::home_dir().ok_or_else(|| {
        crate::utils::error::DotfilesError::Config("Could not find home directory".to_string())
    })?;

    for file in tracked_files {
        // Check sensitive directories
        let sensitive_dirs = [".ssh", ".gnupg", ".aws"];
        let is_sensitive = sensitive_dirs
            .iter()
            .any(|dir| file.dest_path.starts_with(home.join(dir)));

        if is_sensitive && file.dest_path.exists()
            && let Ok(metadata) = fs::metadata(&file.dest_path) {
                let perms = metadata.permissions();
                let mode = perms.mode();
                let file_mode = mode & 0o777;

                // Files in .ssh should be 600, directories 700
                // Files in .gnupg should be 600, directories 700
                if file.dest_path.is_file() && file_mode & 0o077 != 0 {
                    // World or group readable/writable
                    issues.push(HealthIssue::UnsafePermissions {
                        path: file.dest_path.clone(),
                        current_mode: format!("{:o}", file_mode),
                        recommended_mode: "600".to_string(),
                    });
                } else if file.dest_path.is_dir() && file_mode & 0o077 != 0 {
                    issues.push(HealthIssue::UnsafePermissions {
                        path: file.dest_path.clone(),
                        current_mode: format!("{:o}", file_mode),
                        recommended_mode: "700".to_string(),
                    });
                }
            }
    }

    Ok(())
}

/// Display health report with categorized issues and suggested fixes.
pub fn display_health_report(report: &HealthReport) {
    if report.is_healthy() {
        println!("{} All health checks passed!", "✓".green().bold());
        return;
    }

    println!("\n{} Health Check Report", "→".cyan().bold());
    println!("{}", "=".repeat(60).cyan());

    let by_category = report.by_category();
    let categories: Vec<_> = by_category.keys().collect();
    let mut sorted_categories = categories.clone();
    sorted_categories.sort();

    for category in sorted_categories {
        let cat_issues = &by_category[category];
        println!(
            "\n{} {} ({} issue(s))",
            "Category:".bold(),
            category.cyan(),
            cat_issues.len()
        );

        for issue in cat_issues {
            match issue {
                HealthIssue::BrokenSymlink {
                    path,
                    target,
                    reason,
                } => {
                    println!("  {} Broken symlink: {}", "⚠".yellow(), path.display());
                    println!("    Target: {}", target.display());
                    println!("    Reason: {}", reason);
                    println!("    💡 Fix: Run `flux apply` to recreate symlink");
                }
                HealthIssue::MissingRepoFile { file } => {
                    println!(
                        "  {} Missing repo file: {}",
                        "✗".red(),
                        file.repo_path.display()
                    );
                    println!("    💡 Fix: Add file to repository or remove from config");
                }
                HealthIssue::ConfigDrift { file, summary } => {
                    println!(
                        "  {} Config drift: {}",
                        "↻".yellow(),
                        file.dest_path.display()
                    );
                    println!("    {}", summary);
                    println!("    💡 Fix: Run `flux apply` to sync with repository");
                }
                HealthIssue::InvalidProfile { profile, reason } => {
                    println!("  {} Invalid profile: {}", "⚠".yellow(), profile);
                    println!("    Reason: {}", reason);
                    println!("    💡 Fix: Create profile directory or switch to valid profile");
                }
                HealthIssue::BadGitRemote {
                    remote,
                    url,
                    reason,
                } => {
                    println!("  {} Bad git remote: {} ({})", "⚠".yellow(), remote, url);
                    println!("    Reason: {}", reason);
                    println!(
                        "    💡 Fix: Update remote URL with `flux remote set-url {} <new-url>`",
                        remote
                    );
                }
                HealthIssue::UnsafePermissions {
                    path,
                    current_mode,
                    recommended_mode,
                } => {
                    println!("  {} Unsafe permissions: {}", "⚠".yellow(), path.display());
                    println!(
                        "    Current: {}, Recommended: {}",
                        current_mode, recommended_mode
                    );
                    println!(
                        "    💡 Fix: Run `chmod {} {}`",
                        recommended_mode,
                        path.display()
                    );
                }
            }
        }
    }

    println!("\n{}", "=".repeat(60).cyan());
    println!(
        "{} Found {} issue(s) across {} categories",
        "Summary:".bold(),
        report.issues.len().to_string().red(),
        by_category.len()
    );
}
