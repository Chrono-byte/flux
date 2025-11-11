use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::services::{FileOperation, Transaction};
use crate::types::TrackedFile;
use crate::utils::dry_run::DryRun;
use crate::utils::error::{DotfilesError, Result};
use crate::utils::prompt::prompt_yes_no;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Options for applying configuration
#[derive(Debug, Clone)]
pub struct ApplyOptions<'a> {
    /// Configuration to apply
    pub config: &'a Config,
    /// Profile name (optional)
    pub profile: Option<&'a str>,
    /// Dry run mode - preview changes without applying
    pub dry_run: bool,
    /// Auto-confirm without prompting
    pub yes: bool,
    /// Optional description for this apply operation
    pub description: Option<&'a str>,
}

/// State comparison result showing what needs to change
#[derive(Debug, Clone)]
pub struct StateDiff {
    /// Files that need to be synced
    pub files_to_sync: Vec<TrackedFile>,
}

impl StateDiff {
    pub fn new() -> Self {
        Self {
            files_to_sync: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files_to_sync.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.files_to_sync.len()
    }
}

/// Compare declared state (from config) with actual system state
pub fn compare_states(
    config: &Config,
    profile: Option<&str>,
) -> Result<StateDiff> {
    let mut diff = StateDiff::new();

    // Compare files
    let tracked_files = config.get_tracked_files(profile)?;
    for file in tracked_files {
        if needs_sync(&file)? {
            diff.files_to_sync.push(file);
        }
    }

    Ok(diff)
}

/// Check if a file needs to be synced
fn needs_sync(file: &TrackedFile) -> Result<bool> {
    if !file.repo_path.exists() {
        return Ok(false); // Skip if repo file doesn't exist
    }

    if !file.dest_path.exists() && !file.dest_path.is_symlink() {
        return Ok(true); // Destination doesn't exist, needs sync
    }

    // Check if it's a symlink and correctly linked
    if file.dest_path.is_symlink()
        && let Ok(link_target) = fs::read_link(&file.dest_path)
    {
        let resolved_target = if link_target.is_absolute() {
            link_target
        } else {
            file.dest_path
                .parent()
                .map(|p| p.join(&link_target))
                .unwrap_or(link_target)
        };
        let normalized_target = normalize_path(&resolved_target);
        let normalized_repo = normalize_path(&file.repo_path);

        if normalized_target != normalized_repo {
            return Ok(true); // Symlink points to wrong location
        }
        return Ok(false); // Already correctly linked
    }

    // Check if files differ
    if files_differ(&file.repo_path, &file.dest_path)? {
        return Ok(true);
    }

    // Files are identical but dest is not a symlink, should convert
    Ok(true)
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn files_differ(path1: &Path, path2: &Path) -> Result<bool> {
    if !path1.exists() || !path2.exists() {
        return Ok(true);
    }

    if path1.is_dir() || path2.is_dir() {
        return Ok(path1.is_dir() != path2.is_dir());
    }

    let content1 = fs::read(path1)?;
    let content2 = fs::read(path2)?;

    Ok(content1 != content2)
}

/// Display a preview of changes that would be applied
pub fn display_preview(diff: &StateDiff) {
    println!("\n{} Preview of changes:", "→".cyan().bold());

    if diff.files_to_sync.is_empty() {
        println!(
            "  {} System is already in sync with configuration",
            "✓".green()
        );
        return;
    }

    if !diff.files_to_sync.is_empty() {
        println!(
            "\n  {} Files to sync ({}):",
            "📁".cyan(),
            diff.files_to_sync.len()
        );
        for file in &diff.files_to_sync {
            println!("    • {}", file.dest_path.display());
        }
    }

    println!("\n  {} Total changes: {}", "→".cyan(), diff.total_changes());
}

/// Apply configuration changes using atomic transactions
pub fn apply_config(options: ApplyOptions<'_>) -> Result<()> {
    println!("{} Applying configuration...", "→".cyan().bold());

    // Compare states
    let diff = compare_states(
        options.config,
        options.profile,
    )?;

    if diff.is_empty() {
        println!(
            "{} System is already in sync with configuration",
            "✓".green()
        );
        return Ok(());
    }

    // Display preview
    display_preview(&diff);

    // Confirm if not auto-yes
    if !options.dry_run && !options.yes && !prompt_yes_no("Apply these changes?")? {
        println!("{} Apply cancelled", "⊘".yellow());
        return Ok(());
    }

    if options.dry_run {
        println!(
            "\n{} DRY RUN MODE - No changes will be applied",
            "⚠".yellow().bold()
        );
        return Ok(());
    }

    // Create transaction
    let temp_dir = TempDir::new()?.path().to_path_buf();
    let mut transaction = Transaction::begin(
        temp_dir.clone(),
    )?;

    // Add metadata
    if let Some(desc) = options.description {
        transaction
            .metadata
            .insert("description".to_string(), desc.to_string());
    }
    if let Some(prof) = options.profile {
        transaction
            .metadata
            .insert("profile".to_string(), prof.to_string());
    }
    transaction
        .metadata
        .insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());

    // Add file operations
    let symlink_resolution = options.config.general.symlink_resolution;
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;

    // Create a single timestamped backup directory for all files in this transaction
    let backup_dir = options.config.get_backup_dir()?;
    let transaction_backup_dir =
        backup_dir.join(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string());

    for file in &diff.files_to_sync {
        // Check if we need to backup
        if file.dest_path.exists() {
            let backup_path = transaction_backup_dir.join(
                file.dest_path
                    .strip_prefix(&home)
                    .unwrap_or(&file.dest_path),
            );

            transaction.add_operation(FileOperation::BackupAndReplace {
                source: file.repo_path.clone(),
                target: file.dest_path.clone(),
                backup_path: backup_path.clone(),
                resolution: symlink_resolution,
            });
        } else {
            transaction.add_operation(FileOperation::CreateSymlink {
                source: file.repo_path.clone(),
                target: file.dest_path.clone(),
                resolution: symlink_resolution,
            });
        }
    }

    // Execute transaction
    let mut dry_run_tracker = DryRun::new();
    let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, false);

    // Validate
    transaction.validate(options.config)?;

    // Prepare
    transaction.prepare(options.config)?;

    // Commit
    transaction.commit(options.config, &mut fs_manager)?;

    // Verify
    transaction.verify()?;

    // Cleanup
    transaction.cleanup()?;

    println!("\n{} Configuration applied successfully", "✓".green());
    println!("  Transaction ID: {}", transaction.id);

    Ok(())
}
