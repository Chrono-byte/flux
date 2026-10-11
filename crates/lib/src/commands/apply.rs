use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::services::{FileOperation, Transaction, history};
use crate::types::TrackedFile;
use crate::utils::dry_run::DryRun;
use crate::utils::error::{DotfilesError, Result};
use crate::utils::path_utils::{files_differ, symlink_points_to_correct_target};
use std::fs;

type ConfirmCallback<'a> = dyn Fn(&str) -> Result<bool> + 'a;

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
  /// Force sync: replace all files that aren't correct symlinks (uses repo version)
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

/// The outcome of applying configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyStatus {
  /// No tracked files required changes.
  NoChanges,
  /// The apply was cancelled by the user.
  Cancelled,
  /// The apply was run in dry-run mode.
  DryRun,
  /// The apply completed successfully.
  Applied,
}

/// Report returned by `apply_config`.
#[derive(Debug, Clone)]
pub struct ApplyReport {
  /// The diff that was used for this apply run.
  pub diff: StateDiff,
  /// The final status of the apply run.
  pub status: ApplyStatus,
  /// Transaction ID when the apply completed successfully.
  pub transaction_id: Option<String>,
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

  let tracked_files = config.get_tracked_files(profile)?;
  for file in tracked_files {
    if needs_sync(&file, force)? {
      diff.files_to_sync.push(file);
    }
  }

  Ok(diff)
}

fn needs_sync(
  file: &TrackedFile,
  force: bool,
) -> Result<bool> {
  if !file.repo_path.exists() {
    return Ok(false);
  }

  if !file.dest_path.exists() && !file.dest_path.is_symlink() {
    return Ok(true);
  }

  if file.dest_path.is_symlink()
    && let Ok(link_target) = fs::read_link(&file.dest_path)
  {
    if !symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path) {
      return Ok(true);
    }

    if !force {
      return Ok(false);
    }
    return Ok(false);
  }

  if force {
    return Ok(true);
  }

  if files_differ(&file.repo_path, &file.dest_path)? {
    return Ok(true);
  }

  Ok(true)
}

/// Apply configuration changes using atomic transactions
///
/// # Errors
///
/// Returns an error if the configuration cannot be applied.
pub fn apply_config(
  options: &ApplyOptions<'_>,
  confirm: Option<&ConfirmCallback<'_>>,
) -> Result<ApplyReport> {
  let diff = compare_states(options.config, options.profile, options.force)?;

  if diff.is_empty() {
    return Ok(ApplyReport {
      diff,
      status: ApplyStatus::NoChanges,
      transaction_id: None,
    });
  }

  run_pre_apply_hooks(options)?;

  if !options.dry_run && !options.yes {
    let Some(confirm) = confirm else {
      return Err(DotfilesError::Config(
        "apply_config requires an explicit confirmation callback".to_string(),
      ));
    };

    if !confirm("Apply these changes?")? {
      return Ok(ApplyReport {
        diff,
        status: ApplyStatus::Cancelled,
        transaction_id: None,
      });
    }
  }

  if options.dry_run {
    return Ok(ApplyReport {
      diff,
      status: ApplyStatus::DryRun,
      transaction_id: None,
    });
  }

  let mut transaction = create_and_setup_transaction(options, &diff)?;

  let mut dry_run_tracker = DryRun::default();
  let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, false);

  transaction.validate(options.config)?;

  transaction.prepare()?;

  transaction.commit(options.config, &mut fs_manager)?;

  transaction.verify()?;

  if let Err(error) = transaction.cleanup() {
    log::warn!("Applied changes, but failed to clean up transaction files: {error}");
  }

  if let Err(error) = run_post_apply_hooks(options) {
    log::warn!("Applied changes, but post-apply hooks could not be completed: {error}");
  }

  let profile_str = options.profile.map(ToString::to_string);
  let history_event = history::HistoryEvent {
    timestamp: chrono::Utc::now(),
    command: "apply".to_string(),
    profile: profile_str,
    dry_run: false,
    transaction_id: Some(transaction.id.clone()),
    summary: Some(format!("Applied {} file(s)", diff.files_to_sync.len())),
  };
  if let Err(error) = history::record_event(&history_event) {
    log::warn!("Applied changes, but failed to record history: {error}");
  }

  Ok(ApplyReport {
    diff,
    status: ApplyStatus::Applied,
    transaction_id: Some(transaction.id),
  })
}

fn run_pre_apply_hooks(options: &ApplyOptions<'_>) -> Result<()> {
  if !options.dry_run
    && let Some(hooks_config) = &options.config.hooks
    && let Some(pre_hooks) = &hooks_config.pre_apply
  {
    use crate::services::hooks::execute_hooks;
    let hook_results = execute_hooks(pre_hooks, options.config, None, true)?;
    for result in hook_results {
      if !result.success {
        return Err(DotfilesError::Config(format!(
          "Pre-apply hook failed: {} (exit code: {:?})",
          result.command, result.exit_code
        )));
      }

      match (
        result.stderr.is_empty(),
        result.stderr.contains("Hook skipped"),
        result.stdout.is_empty(),
      ) {
        (false, false, _) => {
          return Err(DotfilesError::Config(format!(
            "Pre-apply hook produced stderr: {}",
            result.stderr
          )));
        }
        (_, _, false) => {
          return Err(DotfilesError::Config(format!(
            "Pre-apply hook produced stdout: {}",
            result.stdout
          )));
        }
        _ => {}
      }
    }
  }
  Ok(())
}

fn create_and_setup_transaction(
  options: &ApplyOptions<'_>,
  diff: &StateDiff,
) -> Result<Transaction> {
  let temp_dir = tempfile::tempdir()?.keep();
  let mut transaction = Transaction::begin(temp_dir)?;

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

  let symlink_resolution = options.config.general.symlink_resolution;
  for file in &diff.files_to_sync {
    add_file_operation_to_transaction(&mut transaction, file, symlink_resolution);
  }

  Ok(transaction)
}

fn run_post_apply_hooks(options: &ApplyOptions<'_>) -> Result<()> {
  if let Some(hooks_config) = &options.config.hooks
    && let Some(post_hooks) = &hooks_config.post_apply
  {
    use crate::services::hooks::execute_hooks;
    let hook_results = execute_hooks(post_hooks, options.config, None, false)?;
    for result in hook_results {
      if !result.success {
        log::warn!(
          "Post-apply hook failed (non-fatal): {} (exit code: {:?})",
          result.command,
          result.exit_code
        );
      }
    }
  }
  Ok(())
}

fn add_file_operation_to_transaction(
  transaction: &mut Transaction,
  file: &TrackedFile,
  symlink_resolution: crate::types::SymlinkResolution,
) {
  transaction.add_operation(FileOperation::CreateSymlink {
    source: file.repo_path.clone(),
    target: file.dest_path.clone(),
    resolution: symlink_resolution,
  });
}
