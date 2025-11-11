use crate::config::Config;
use crate::utils::error::Result;
use crate::types::TrackedFile;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Discrepancy {
    pub file: TrackedFile,
    pub issue: IssueType,
    pub message: String,
}

#[derive(Debug, Clone)]
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

pub fn find_discrepancies(config: &Config, profile: Option<&str>) -> Result<Vec<Discrepancy>> {
    let tracked_files = config.get_tracked_files(profile)?;
    let mut discrepancies = Vec::new();

    for file in tracked_files {
        if let Some(discrepancy) = check_file_discrepancy(&file)? {
            discrepancies.push(discrepancy);
        }
    }

    // Sort by tool, then by path
    discrepancies.sort_by(|a, b| {
        a.file
            .tool
            .cmp(&b.file.tool)
            .then_with(|| a.file.dest_path.cmp(&b.file.dest_path))
    });

    Ok(discrepancies)
}

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
                // Check if symlink is broken
                if !link_target.exists() {
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
                // Normalize paths for comparison
                let normalized_target = normalize_path(&link_target);
                let normalized_repo = normalize_path(&file.repo_path);

                if normalized_target != normalized_repo {
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
                // Content comparison is not needed for symlinks
                Ok(None)
            }
            Err(e) => {
                // Can't read symlink - treat as broken
                Ok(Some(Discrepancy {
                    file: file.clone(),
                    issue: IssueType::BrokenSymlink,
                    message: format!("Cannot read symlink: {} ({})", file.dest_path.display(), e),
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

fn normalize_path(path: &Path) -> PathBuf {
    // Try to canonicalize, but fall back to the path itself if it fails
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn files_differ(path1: &Path, path2: &Path) -> Result<bool> {
    if !path1.exists() || !path2.exists() {
        return Ok(true);
    }

    // If either is a directory, we can't compare contents directly
    if path1.is_dir() || path2.is_dir() {
        // For directories, we consider them different if one is dir and other isn't
        return Ok(path1.is_dir() != path2.is_dir());
    }

    let content1 = fs::read(path1)?;
    let content2 = fs::read(path2)?;

    Ok(content1 != content2)
}

pub fn display_discrepancies(discrepancies: &[Discrepancy]) {
    if discrepancies.is_empty() {
        println!(
            "{}",
            "✓ All tracked files are correctly configured.".green()
        );
        return;
    }

    println!("\n{}", "Discrepancies Found:".bold().red());
    println!("{}", "=".repeat(80).red());

    // Group by tool
    use std::collections::HashMap;
    let mut by_tool: HashMap<String, Vec<&Discrepancy>> = HashMap::new();
    for discrepancy in discrepancies {
        by_tool
            .entry(discrepancy.file.tool.clone())
            .or_default()
            .push(discrepancy);
    }

    let mut tool_names: Vec<_> = by_tool.keys().collect();
    tool_names.sort();

    for tool in tool_names {
        let tool_discrepancies = &by_tool[tool];
        println!(
            "\n{} {} ({} issue(s))",
            "Tool:".bold().yellow(),
            tool.cyan(),
            tool_discrepancies.len()
        );

        for discrepancy in tool_discrepancies {
            let icon = match discrepancy.issue {
                IssueType::Missing => "⊘".red(),
                IssueType::NotSymlink => "⚠".yellow(),
                IssueType::WrongTarget => "↻".yellow(),
                IssueType::ContentDiffers => "↻".yellow(),
                IssueType::MissingRepo => "✗".red(),
                IssueType::BrokenSymlink => "⚠".red(),
            };

            let profile_info = if let Some(profile) = &discrepancy.file.profile {
                format!(" [profile: {}]", profile).yellow()
            } else {
                "".normal()
            };

            println!("  {} {}", icon, discrepancy.message.bright_white());
            println!(
                "      Repo: {}",
                discrepancy
                    .file
                    .repo_path
                    .display()
                    .to_string()
                    .bright_black()
            );
            println!(
                "      Dest: {}{}",
                discrepancy
                    .file
                    .dest_path
                    .display()
                    .to_string()
                    .bright_black(),
                profile_info
            );
        }
    }

    println!("\n{}", "=".repeat(80).red());
    println!(
        "{} Found {} discrepancy(ies) in tracked files",
        "→".cyan(),
        discrepancies.len()
    );
    println!(
        "\n{}",
        "Tip: Run 'sync' command to fix these issues"
            .yellow()
            .italic()
    );
}
