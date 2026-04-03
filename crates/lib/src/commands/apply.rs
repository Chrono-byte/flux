use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::services::{FileOperation, Transaction, history, unified_diff};
use crate::types::TrackedFile;
use crate::utils::dry_run::DryRun;
use crate::utils::error::{DotfilesError, Result};
use crate::utils::home_override;
use crate::utils::path_utils::{files_differ, symlink_points_to_correct_target};
use crate::utils::prompt::prompt_yes_no;
use colored::Colorize;
use std::fmt::Write;
use std::fs;
use std::path::Path;
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
  /// Force sync: replace all files that aren't correct symlinks (no backups)
  pub force: bool,
}

/// Difference between declared and actual system state.
#[derive(Debug, Clone, Default)]
pub struct StateDiff {
  /// Files that need to be synced to match configuration
  pub files_to_sync: Vec<TrackedFile>,
}

impl StateDiff {
  /// Check if the state diff is empty.
  ///
  /// # Returns
  ///
  /// Returns true if the state diff is empty, false otherwise.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.files_to_sync.is_empty()
  }

  /// Get the total number of changes in the state diff.
  #[must_use]
  pub const fn total_changes(&self) -> usize {
    self.files_to_sync.len()
  }
}

/// Compare declared state (from config) with actual system state
///
/// # Errors
///
/// Returns an error if the configuration file cannot be loaded.
pub fn compare_states(
  config: &Config,
  profile: Option<&str>,
  force: bool,
) -> Result<StateDiff> {
  let mut diff = StateDiff::default();

  // Compare files
  let tracked_files = config.get_tracked_files(profile)?;
  for file in tracked_files {
    if needs_sync(&file, force)? {
      diff.files_to_sync.push(file);
    }
  }

  Ok(diff)
}

/// Check if a file needs to be synced
fn needs_sync(
  file: &TrackedFile,
  force: bool,
) -> Result<bool> {
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
    if !symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path) {
      return Ok(true); // Symlink points to wrong location
    }

    // In force mode, we still sync if it's not a symlink, but if it's correctly linked, we're done
    if !force {
      return Ok(false); // Already correctly linked
    }
    // In force mode, if it's correctly linked, no need to sync
    return Ok(false);
  }

  // If force mode, always sync if it's not a correct symlink (don't check content)
  if force {
    return Ok(true); // Not a symlink or wrong symlink, force replace
  }

  // Check if files differ
  if files_differ(&file.repo_path, &file.dest_path)? {
    return Ok(true);
  }

  // Files are identical but dest is not a symlink, should convert
  Ok(true)
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

/// Format unified diffs for files that would be changed.
///
/// This is only used in dry-run mode and never touches the filesystem.
#[must_use]
pub fn format_diffs(diff: &StateDiff) -> String {
  if diff.files_to_sync.is_empty() {
    return String::new();
  }

  let mut out = String::new();
  let _ = writeln!(
    out,
    "\n{}",
    "Unified diffs for planned changes:".bold().cyan()
  );
  let _ = writeln!(out, "{}", "=".repeat(60).cyan());

  for file in &diff.files_to_sync {
    let current = fs::read_to_string(&file.dest_path).unwrap_or_default();

    let Ok(target) = fs::read_to_string(&file.repo_path) else {
      let _ = writeln!(
        out,
        "\n{} {} (skipping diff: cannot read repo file)",
        "⊘".yellow(),
        file.repo_path.display()
      );
      continue;
    };

    let udiff = unified_diff(&file.dest_path, &current, &target);
    if udiff.trim().is_empty() {
      continue;
    }

    let _ = writeln!(out, "\n{} {}", "File:".bold(), file.dest_path.display());
    let _ = writeln!(out, "{}", "-".repeat(60).cyan());
    let _ = writeln!(out, "{udiff}");
  }

  out
}

/// Display unified diffs for files that would be changed.
///
/// This is only used in dry-run mode and never touches the filesystem.
pub fn display_diffs(diff: &StateDiff) {
  let output = format_diffs(diff);
  if output.is_empty() {
    return;
  }
  print!("{output}");
}

/// Apply configuration changes using atomic transactions
///
/// # Errors
///
/// Returns an error if the configuration cannot be applied.
pub fn apply_config(options: &ApplyOptions<'_>) -> Result<()> {
  if options.force {
    println!(
      "{} Force applying configuration (no backups, using repo version)...",
      "→".cyan().bold()
    );
  } else {
    println!("{} Applying configuration...", "→".cyan().bold());
  }

  // Compare states
  let diff = compare_states(options.config, options.profile, options.force)?;

  if diff.is_empty() {
    println!(
      "{} System is already in sync with configuration",
      "✓".green()
    );
    return Ok(());
  }

  // Display preview
  display_preview(&diff);

  // Run pre-apply hooks (if not dry-run)
  run_pre_apply_hooks(options)?;

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

  // Create and setup transaction
  let mut transaction = create_and_setup_transaction(options, &diff)?;

  // Execute transaction
  let mut dry_run_tracker = DryRun::default();
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

  // Run post-apply hooks
  run_post_apply_hooks(options)?;

  // Record history event
  let profile_str = options.profile.map(ToString::to_string);
  history::record_event(&history::HistoryEvent {
    timestamp: chrono::Utc::now(),
    command: "apply".to_string(),
    profile: profile_str,
    dry_run: false,
    transaction_id: Some(transaction.id.clone()),
    summary: Some(format!("Applied {} file(s)", diff.files_to_sync.len())),
  })?;

  println!("\n{} Configuration applied successfully", "✓".green());
  println!("  Transaction ID: {}", transaction.id);

  Ok(())
}

/// Run pre-apply hooks if configured.
fn run_pre_apply_hooks(options: &ApplyOptions<'_>) -> Result<()> {
  if !options.dry_run
    && let Some(hooks_config) = &options.config.hooks
    && let Some(pre_hooks) = &hooks_config.pre_apply
  {
    use crate::services::hooks::execute_hooks;
    println!("{} Running pre-apply hooks...", "→".cyan());
    let hook_results = execute_hooks(pre_hooks, options.config, None, true)?;
    for result in hook_results {
      if !result.success {
        return Err(DotfilesError::Config(format!(
          "Pre-apply hook failed: {} (exit code: {:?})",
          result.command, result.exit_code
        )));
      }
      if !result.stdout.is_empty() {
        print!("{}", result.stdout);
      }
      if !result.stderr.is_empty() && !result.stderr.contains("Hook skipped") {
        eprint!("{}", result.stderr);
      }
    }
  }
  Ok(())
}

/// Create and setup transaction with all file operations.
fn create_and_setup_transaction(
  options: &ApplyOptions<'_>,
  diff: &StateDiff,
) -> Result<Transaction> {
  let temp_dir = TempDir::new()?.path().to_path_buf();
  let mut transaction = Transaction::begin(temp_dir)?;

  // Add metadata
  if let Some(desc) = options.description {
    let _ = transaction
      .metadata
      .insert("description".to_string(), desc.to_string());
  }
  if let Some(prof) = options.profile {
    let _ = transaction
      .metadata
      .insert("profile".to_string(), prof.to_string());
  }
  let _ = transaction
    .metadata
    .insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());

  // Add file operations
  let symlink_resolution = options.config.general.symlink_resolution;
  let home = home_override::home_dir()
    .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;

  // Create a single timestamped backup directory for all files in this transaction
  let backup_dir = options.config.get_backup_dir()?;
  let transaction_backup_dir = backup_dir.join(
    chrono::Local::now()
      .format("%Y%m%d_%H%M%S")
      .to_string(),
  );

  for file in &diff.files_to_sync {
    add_file_operation_to_transaction(
      &mut transaction,
      file,
      options.force,
      symlink_resolution,
      &home,
      &transaction_backup_dir,
    );
  }

  Ok(transaction)
}

/// Run post-apply hooks if configured.
fn run_post_apply_hooks(options: &ApplyOptions<'_>) -> Result<()> {
  if let Some(hooks_config) = &options.config.hooks
    && let Some(post_hooks) = &hooks_config.post_apply
  {
    use crate::services::hooks::execute_hooks;
    println!("{} Running post-apply hooks...", "→".cyan());
    let hook_results = execute_hooks(post_hooks, options.config, None, false)?; // Don't fail-fast on post hooks
    for result in hook_results {
      if !result.success {
        eprintln!(
          "{} Post-apply hook failed (non-fatal): {} (exit code: {:?})",
          "⚠".yellow(),
          result.command,
          result.exit_code
        );
      }
      if !result.stdout.is_empty() {
        print!("{}", result.stdout);
      }
      if !result.stderr.is_empty() && !result.stderr.contains("Hook skipped") {
        eprint!("{}", result.stderr);
      }
    }
  }
  Ok(())
}

/// Add file operation to transaction based on force mode and file state.
fn add_file_operation_to_transaction(
  transaction: &mut Transaction,
  file: &TrackedFile,
  force: bool,
  symlink_resolution: crate::types::SymlinkResolution,
  home: &Path,
  backup_dir: &Path,
) {
  if force {
    // Force mode: no backups, just remove and create symlink
    if file.dest_path.exists() {
      transaction.add_operation(FileOperation::RemoveSymlink {
        target: file.dest_path.clone(),
      });
    }
    transaction.add_operation(FileOperation::CreateSymlink {
      source: file.repo_path.clone(),
      target: file.dest_path.clone(),
      resolution: symlink_resolution,
    });
  } else {
    // Normal mode: backup existing files
    if file.dest_path.exists() {
      let relative_path = match file.dest_path.strip_prefix(home) {
        Ok(rel) => rel,
        Err(_) => &file.dest_path,
      };
      let backup_path = backup_dir.join(relative_path);

      transaction.add_operation(FileOperation::BackupAndReplace {
        source: file.repo_path.clone(),
        target: file.dest_path.clone(),
        backup_path,
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
}
