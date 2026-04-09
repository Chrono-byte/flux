use crate::config::Config;
use crate::types::{SymlinkResolution, TrackedFile};
use crate::utils::dry_run::{DryRun, Operation};
use crate::utils::error::{DotfilesError, Result};
use crate::utils::home_override;
use crate::utils::path_utils::{
  files_differ, find_symlinked_parent, normalize_path, symlink_points_to_correct_target,
};
use crate::utils::prompt::ConflictChoice;
use crate::utils::security;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, warn};
use std::fs;
use std::path::Path;

/// Callback used by the CLI to resolve file sync conflicts.
type ConflictPromptCallback<'a> = dyn Fn(&Path) -> Result<ConflictChoice> + 'a;

/// Summary of a sync operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncReport {
  /// Number of files synced.
  pub synced: usize,
  /// Number of files skipped.
  pub skipped: usize,
}

// #############################################################################
// ## Public API Functions
// #############################################################################

/// Add a file to the dotfiles repository.
///
/// # Errors
///
/// Returns an error if there are issues validating paths, copying files,
/// or updating the configuration.
pub fn add_file(
  config: &mut Config,
  tool: &str,
  source_path: &Path,
  dest_path: &Path,
  profile: Option<&str>,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  let home = home_override::home_dir().ok_or_else(crate::utils::error_utils::home_dir_not_found)?;

  // Validate destination path is within home directory
  security::validate_dest_path(dest_path, &home)?;

  let repo_path = config.get_repo_path();
  let tool_dir = repo_path.join(tool);
  let repo_file = tool_dir.join(source_path.file_name().ok_or_else(|| {
    DotfilesError::Path(format!(
      "What: Source path has no file name component\n  \
                     Path: {}\n  \
                     Why: Path ends with '/' or is empty, preventing file extraction\n  \
                     💡 Solution: Specify a file path without trailing slashes",
      source_path.display()
    ))
  })?);
  let is_directory = source_path.is_dir();

  // Copy source file or directory to repository
  fs_manager.create_dir_all(&tool_dir)?;

  if is_directory {
    fs_manager.copy_dir_all(source_path, &repo_file)?;
  } else {
    fs_manager.copy(source_path, &repo_file)?;
  }

  // Register file in configuration (in-memory, will be saved later)
  let repo_relative = repo_file
    .strip_prefix(&repo_path)
    .map_err(|error| {
      crate::utils::error_utils::invalid_path_computation(
        &repo_path,
        &repo_file,
        &format!("Repository file is not within repository directory: {error}"),
      )
    })?
    .to_string_lossy()
    .to_string();
  if is_directory {
    config.add_folder_to_tool(tool, &repo_relative, dest_path, profile);
  } else {
    config.add_file_to_tool(tool, &repo_relative, dest_path, profile);
  }

  // Only save config if not in dry run mode
  if fs_manager.is_dry_run {
  } else {
    config.save(fs_manager.is_dry_run)?;
  }

  Ok(())
}

/// Sync all tracked files, creating symlinks from repo to destination.
///
/// # Errors
///
/// Returns an error if there are issues reading configuration, creating symlinks,
/// or managing files.
pub fn sync_files(
  config: &Config,
  profile: Option<&str>,
  dry_run_tracker: &mut DryRun,
  is_dry_run_mode: bool,
  conflict_prompt: Option<&ConflictPromptCallback<'_>>,
) -> Result<SyncReport> {
  let tracked_files = config.get_tracked_files(profile)?;
  let symlink_resolution = config.general.symlink_resolution;

  if tracked_files.is_empty() {
    return Ok(SyncReport::default());
  }

  let progress_bar = if !is_dry_run_mode && tracked_files.len() > 1 {
    let pb = ProgressBar::new(tracked_files.len() as u64);
    pb.set_style(
      ProgressStyle::default_bar()
        .template(
          "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({msg})",
        )
        .map_err(|_| DotfilesError::Config("Failed to create progress bar".to_string()))?
        .progress_chars("#>-"),
    );
    Some(pb)
  } else {
    None
  };

  // Create FileSystemManager to handle all file operations with dry-run support
  let mut fs_manager = FileSystemManager::new(dry_run_tracker, is_dry_run_mode);

  // Process files sequentially with progress bar
  let mut report = SyncReport::default();
  for file in &tracked_files {
    if let Some(ref pb) = progress_bar {
      pb.set_message(
        file
          .dest_path
          .file_name()
          .and_then(|n| n.to_str())
          .unwrap_or("file")
          .to_string(),
      );
    }

    match sync_file(file, symlink_resolution, &mut fs_manager, conflict_prompt)? {
      SyncResult::Synced => report.synced += 1,
      SyncResult::Skipped => report.skipped += 1,
    }

    if let Some(ref pb) = progress_bar {
      pb.inc(1);
    }
  }

  // Finish progress bar
  if let Some(pb) = progress_bar {
    pb.finish_with_message("Sync complete");
  }

  Ok(report)
}

/// Result of a file synchronization operation.
#[derive(Clone, Copy)]
enum SyncResult {
  /// File was successfully synced.
  Synced,
  /// File was skipped during sync.
  Skipped,
}

/// Remove a file from tracking and delete it from the filesystem.
///
/// # Errors
///
/// Returns an error if the file is not found, or if there are issues
/// removing files or updating configuration.
pub fn remove_file(
  config: &mut Config,
  tool: &str,
  file: &str,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  let (file_entry, is_folder) = {
    let tool_config = config
      .tools
      .get(tool)
      .ok_or_else(|| DotfilesError::InvalidTool(tool.to_string()))?;

    if let Some(entry) = tool_config
      .files
      .iter()
      .find(|entry| entry.repo == file)
    {
      (entry.clone(), false)
    } else if let Some(entry) = tool_config
      .folders
      .iter()
      .find(|entry| entry.repo == file)
    {
      (entry.clone(), true)
    } else {
      return Err(DotfilesError::Path(format!(
        "File {file} not found in tool {tool}"
      )));
    }
  };

  let dest_path = home_override::home_dir()
    .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?
    .join(&file_entry.dest);

  // Remove symlink or file from destination location
  fs_manager.remove_file(&dest_path)?;

  // Remove file from repository
  let repo_path = config.get_repo_path();
  // Handle both path formats: file may or may not include the tool name prefix
  let repo_file = if file.starts_with(&format!("{tool}/")) {
    // File path already includes tool name (e.g., "cursor/settings.json")
    repo_path.join(file)
  } else {
    // File path doesn't include tool name (e.g., "config")
    repo_path.join(tool).join(file)
  };
  fs_manager.remove_file(&repo_file)?;

  // Remove file entry from configuration (in-memory, will be saved later)
  if let Some(tool_config) = config.tools.get_mut(tool) {
    if is_folder {
      tool_config
        .folders
        .retain(|entry| entry.repo != file);
    } else {
      tool_config
        .files
        .retain(|entry| entry.repo != file);
    }

    if tool_config.files.is_empty() && tool_config.folders.is_empty() {
      let _ = config.tools.remove(tool);
    }
  }

  // Save configuration to disk only if not in dry run mode
  if fs_manager.is_dry_run {
  } else {
    config.save(fs_manager.is_dry_run)?;
  }

  Ok(())
}

// #############################################################################
// ## FileSystemManager Struct
// #############################################################################

/// Manages file system operations with dry-run support.
#[derive(Debug)]
pub struct FileSystemManager<'a> {
  /// Dry-run tracker for logging operations.
  dry_run: &'a mut DryRun,
  /// Whether operations are in dry-run mode
  pub is_dry_run: bool,
}

impl<'a> FileSystemManager<'a> {
  /// Create a new `FileSystemManager`.
  pub const fn new(
    dry_run: &'a mut DryRun,
    is_dry_run: bool,
  ) -> Self {
    Self {
      dry_run,
      is_dry_run,
    }
  }

  /// Create a directory and all parent directories.
  ///
  /// # Errors
  ///
  /// Returns an error if the directory cannot be created.
  pub fn create_dir_all(
    &mut self,
    path: &Path,
  ) -> Result<()> {
    if self.is_dry_run {
      if path.exists() && path.is_dir() {
        return Ok(());
      }
      self
        .dry_run
        .log_operation(Operation::CreateDirectory {
          path: path.to_path_buf(),
        });
      Ok(())
    } else {
      fs::create_dir_all(path).map_err(Into::into)
    }
  }

  /// Copy a file from one location to another.
  ///
  /// # Errors
  ///
  /// Returns an error if the file cannot be copied.
  pub fn copy(
    &mut self,
    from: &Path,
    to: &Path,
  ) -> Result<()> {
    // Safety check: don't copy a file to itself
    if from == to {
      return Err(DotfilesError::Path(format!(
        "Cannot copy file to itself: {}",
        from.display()
      )));
    }

    if self.is_dry_run {
      self.dry_run.log_operation(Operation::CopyFile {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
      });
      Ok(())
    } else {
      fs::copy(from, to).map(|_| ()).map_err(Into::into)
    }
  }

  /// Recursively copy a directory.
  ///
  /// # Errors
  ///
  /// Returns an error if the directory cannot be copied.
  pub fn copy_dir_all(
    &mut self,
    src: &Path,
    dst: &Path,
  ) -> Result<()> {
    // Safety check: don't copy a directory to itself
    if src == dst {
      return Err(DotfilesError::Path(format!(
        "Cannot copy directory to itself: {}",
        src.display()
      )));
    }

    if self.is_dry_run {
      self.dry_run.log_operation(Operation::CopyFile {
        from: src.to_path_buf(),
        to: dst.to_path_buf(),
      });
      Ok(())
    } else {
      copy_dir_all(src, dst)
    }
  }

  /// Remove a file or symlink.
  ///
  /// # Errors
  ///
  /// Returns an error if the file cannot be removed.
  pub fn remove_file(
    &mut self,
    path: &Path,
  ) -> Result<()> {
    if self.is_dry_run {
      self.dry_run.log_operation(Operation::RemoveFile {
        path: path.to_path_buf(),
      });
      Ok(())
    } else {
      if !path.exists() && !path.is_symlink() {
        return Ok(());
      }

      if path.is_symlink() {
        fs::remove_file(path).map_err(Into::into)
      } else if path.is_dir() {
        fs::remove_dir_all(path).map_err(Into::into)
      } else {
        fs::remove_file(path).map_err(Into::into)
      }
    }
  }

  /// Rename or move a file or directory.
  ///
  /// # Errors
  ///
  /// Returns an error if the file cannot be renamed.
  pub fn rename(
    &mut self,
    from: &Path,
    to: &Path,
  ) -> Result<()> {
    if self.is_dry_run {
      self.dry_run.log_operation(Operation::CopyFile {
        // Note: Using CopyFile operation for rename (could add dedicated Rename operation)
        from: from.to_path_buf(),
        to: to.to_path_buf(),
      });
      Ok(())
    } else {
      fs::rename(from, to).map_err(Into::into)
    }
  }

  /// Create a symbolic link.
  ///
  /// # Errors
  ///
  /// Returns an error if symlink cannot be created.
  pub fn symlink(
    &mut self,
    from: &Path,
    to: &Path,
  ) -> Result<()> {
    if self.is_dry_run {
      self
        .dry_run
        .log_operation(Operation::CreateSymlink {
          from: from.to_path_buf(),
          to: to.to_path_buf(),
        });
      Ok(())
    } else {
      // Cross-platform symlink creation
      #[cfg(unix)]
      {
        use std::os::unix::fs::symlink;
        symlink(from, to).map_err(Into::into)
      }
      #[cfg(windows)]
      {
        // Windows requires separate functions for file vs directory symlinks
        if from.is_dir() {
          std::os::windows::fs::symlink_dir(from, to).map_err(Into::into)
        } else {
          std::os::windows::fs::symlink_file(from, to).map_err(Into::into)
        }
      }
      #[cfg(not(any(unix, windows)))]
      {
        Err(DotfilesError::Io(std::io::Error::new(
          std::io::ErrorKind::Unsupported,
          "Symlinking is not supported on this platform",
        )))
      }
    }
  }
}

// #############################################################################
// ## Sync Logic (Decomposed)
// #############################################################################

/// Enum representing the action to take during a sync.
enum SyncAction {
  /// Already correctly linked
  DoNothing,
  /// Destination doesn't exist, or is identical and not a symlink
  CreateSymlink,
  /// Safety check: Repo is empty, dest has content.
  UpdateRepoFromDest,
  /// Files differ, or symlink is wrong.
  ResolveConflict,
}

/// Orchestrates the sync for a single file.
fn sync_file(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
  conflict_prompt: Option<&ConflictPromptCallback<'_>>,
) -> Result<SyncResult> {
  // Step 1: Precondition checks
  if !file.repo_path.exists() {
    return Ok(SyncResult::Skipped);
  }

  // Check if destination file is locked (e.g., in use by another process)
  if let Some(skip_reason) = check_file_lock_status(&file.dest_path)? {
    return Ok(skip_reason);
  }

  // Step 3: Determine what action to take
  let action = determine_sync_action(file)?;

  // Step 4: Execute the determined action
  match action {
    SyncAction::DoNothing => {
      Ok(SyncResult::Skipped) // Already correctly linked
    }
    SyncAction::CreateSymlink => {
      create_symlink_managed(file, resolution, fs_manager)?;
      Ok(SyncResult::Synced)
    }
    SyncAction::UpdateRepoFromDest => {
      if let Some(parent) = file.repo_path.parent() {
        fs_manager.create_dir_all(parent)?;
      }
      if file.dest_path.is_dir() {
        fs_manager.copy_dir_all(&file.dest_path, &file.repo_path)?;
      } else {
        fs_manager.copy(&file.dest_path, &file.repo_path)?;
      }

      // Now that repo is updated, create the symlink
      create_symlink_managed(file, resolution, fs_manager)?;
      Ok(SyncResult::Synced)
    }
    SyncAction::ResolveConflict => {
      handle_file_conflict(file, resolution, fs_manager, conflict_prompt)?;
      Ok(SyncResult::Synced)
    }
  }
}

/// Determines what action to take for a file. (No side-effects)
fn determine_sync_action(file: &TrackedFile) -> Result<SyncAction> {
  if !file.dest_path.exists() && !file.dest_path.is_symlink() {
    return Ok(SyncAction::CreateSymlink);
  }

  // Check if it's a symlink and already correctly linked
  if file.dest_path.is_symlink()
    && let Ok(link_target) = fs::read_link(&file.dest_path)
  {
    if symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path) {
      return Ok(SyncAction::DoNothing);
    }
    return Ok(SyncAction::ResolveConflict);
  }

  // Check if a parent directory is symlinked - if so, we need to convert it
  if let Some((symlinked_parent, resolved_parent)) = find_symlinked_parent(&file.dest_path) {
    // Get the relative path from the symlinked parent to the file
    if let Ok(relative) = file.dest_path.strip_prefix(&symlinked_parent) {
      let resolved_file_path = resolved_parent.join(relative);
      let normalized_resolved = normalize_path(&resolved_file_path);
      let normalized_repo = normalize_path(&file.repo_path);

      // If the resolved path through the symlinked parent matches the repo path,
      // we need to convert the directory symlink to individual file symlinks
      if normalized_resolved == normalized_repo {
        // Files are effectively the same (via directory symlink), convert to individual symlink
        return Ok(SyncAction::CreateSymlink);
      }
    }
  }

  // Safety check: prevent overwriting non-empty destination with empty repo file
  let repo_is_empty = if file.repo_path.is_file() {
    fs::metadata(&file.repo_path).is_ok_and(|metadata| metadata.len() == 0)
  } else {
    false // Dirs aren't "empty" for this check
  };
  let dest_has_content = if file.dest_path.is_file() {
    fs::metadata(&file.dest_path).is_ok_and(|metadata| metadata.len() > 0)
  } else {
    false
  };

  if repo_is_empty && dest_has_content {
    return Ok(SyncAction::UpdateRepoFromDest);
  }

  // Check if files are different
  if files_differ(&file.repo_path, &file.dest_path)? {
    Ok(SyncAction::ResolveConflict)
  } else {
    // Files are identical, but dest is not a symlink. Convert it.
    Ok(SyncAction::CreateSymlink)
  }
}

/// Handles the user-interactive part of resolving a file conflict.
fn handle_file_conflict(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
  conflict_prompt: Option<&ConflictPromptCallback<'_>>,
) -> Result<()> {
  let conflict_choice = if fs_manager.is_dry_run {
    ConflictChoice::UseRepo
  } else {
    // Prompt user for conflict resolution
    let Some(conflict_prompt) = conflict_prompt else {
      return Err(DotfilesError::Config(
        "sync_files requires an explicit conflict prompt callback".to_string(),
      ));
    };
    conflict_prompt(&file.dest_path)?
  };

  match conflict_choice {
    ConflictChoice::UseRepo => {
      create_symlink_managed(file, resolution, fs_manager)?;
    }
    ConflictChoice::KeepLocal => {
      return Ok(());
    }
    ConflictChoice::ViewDiff => {
      if fs_manager.is_dry_run {
        create_symlink_managed(file, resolution, fs_manager)?;
      } else {
        show_diff(&file.repo_path, &file.dest_path)?;
        let Some(conflict_prompt) = conflict_prompt else {
          return Err(DotfilesError::Config(
            "sync_files requires an explicit conflict prompt callback".to_string(),
          ));
        };
        let post_diff_choice = conflict_prompt(&file.dest_path)?;
        match post_diff_choice {
          ConflictChoice::UseRepo => {
            create_symlink_managed(file, resolution, fs_manager)?;
          }
          ConflictChoice::KeepLocal | ConflictChoice::ViewDiff => {}
          ConflictChoice::Abort => {
            return Err(DotfilesError::Cancelled);
          }
        }
      }
    }
    ConflictChoice::Abort => {
      return Err(DotfilesError::Cancelled);
    }
  }
  Ok(())
}

/// Creates a symlink, managed by the `FileSystemManager`.
fn create_symlink_managed(
  file: &TrackedFile,
  resolution: SymlinkResolution,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  validate_symlink_security(file)?;

  debug!(
    "Creating symlink from {} to {}",
    file.dest_path.display(),
    file.repo_path.display()
  );

  ensure_parent_directory(file, fs_manager)?;

  // Handle "Replace" strategy: copy file instead of creating symlink
  if resolution == SymlinkResolution::Replace {
    handle_replace_strategy(file, fs_manager)?;
    return Ok(());
  }

  // Handle regular symlink creation (non-Replace strategies)
  let link_target = compute_symlink_target(file, resolution)?;
  create_symlink_atomic(file, &link_target, fs_manager)?;

  Ok(())
}

/// Validates symlink security constraints.
fn validate_symlink_security(file: &TrackedFile) -> Result<()> {
  if let Err(error) = security::validate_symlink_target(&file.repo_path, &file.repo_path) {
    warn!(
      "Symlink validation failed for {}: {}",
      file.repo_path.display(),
      error
    );
    return Err(error);
  }
  Ok(())
}

/// Ensures the parent directory exists, handling symlinked parent cases.
fn ensure_parent_directory(
  file: &TrackedFile,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  if let Some(parent) = file.dest_path.parent() {
    // Check if parent is a symlink pointing to repo - if so, warn but continue
    fs_manager.create_dir_all(parent)?;
  }
  Ok(())
}

/// Handles the Replace strategy by copying files instead of creating symlinks.
fn handle_replace_strategy(
  file: &TrackedFile,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  let temp_path = file.dest_path.with_extension("flux-temp-copy");

  fs_manager.copy(&file.repo_path, &temp_path)?;
  fs_manager.rename(&temp_path, &file.dest_path)?; // Atomic move
  Ok(())
}

/// Computes the symlink target path based on the resolution strategy.
fn compute_symlink_target(
  file: &TrackedFile,
  resolution: SymlinkResolution,
) -> Result<std::path::PathBuf> {
  let link_target = match resolution {
    SymlinkResolution::Auto => file.dest_path.parent().map_or_else(
      || file.repo_path.clone(),
      |parent| {
        pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
      },
    ),
    SymlinkResolution::Relative => {
      let parent = file.dest_path.parent().ok_or_else(|| {
        DotfilesError::Path("Cannot determine parent for destination path".to_string())
      })?;
      pathdiff::diff_paths(&file.repo_path, parent)
        .ok_or_else(|| DotfilesError::Path("Cannot create relative symlink".to_string()))?
    }
    SymlinkResolution::Absolute => file.repo_path.clone(),
    SymlinkResolution::Follow => file.dest_path.parent().map_or_else(
      || file.repo_path.clone(),
      |parent| {
        pathdiff::diff_paths(&file.repo_path, parent).unwrap_or_else(|| file.repo_path.clone())
      },
    ),
    SymlinkResolution::Replace => unreachable!(), // Handled earlier
  };
  Ok(link_target)
}

/// Creates a symlink atomically using a temporary path.
fn create_symlink_atomic(
  file: &TrackedFile,
  link_target: &Path,
  fs_manager: &mut FileSystemManager,
) -> Result<()> {
  let ext_str = file
    .dest_path
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap_or_default();
  let temp_link_path = file
    .dest_path
    .with_extension(format!("{ext_str}.flux-temp"));

  // Remove any existing temporary symlink from previous failed operations
  fs_manager.remove_file(&temp_link_path)?;
  fs_manager.symlink(link_target, &temp_link_path)?;

  // Atomically rename temporary symlink to final destination
  fs_manager.rename(&temp_link_path, &file.dest_path)?;

  Ok(())
}

// #############################################################################
// ## Module-Private Helpers
// #############################################################################

/// Check if a file is locked and handle the result.
///
/// Returns `Some(SyncResult::Skipped)` if the file is locked and should be skipped,
/// or `None` if the file is not locked or lock check failed (continues processing).
fn check_file_lock_status(file_path: &Path) -> Result<Option<SyncResult>> {
  if !file_path.exists() {
    return Ok(None);
  }

  match security::is_file_locked(file_path) {
    Ok(true) => {
      warn!(
        "File {} is locked (may be in use), skipping",
        file_path.display()
      );
      Ok(Some(SyncResult::Skipped))
    }
    Ok(false) => {
      debug!("File {} is not locked", file_path.display());
      Ok(None)
    }
    Err(error) => Err(error),
  }
}

/// Recursively copy a directory.
fn copy_dir_all(
  src: &Path,
  dst: &Path,
) -> Result<()> {
  fs::create_dir_all(dst)?;

  for entry in fs::read_dir(src)? {
    let entry = entry?;
    let path = entry.path();
    let file_name = entry.file_name();
    let dst_path = dst.join(&file_name);

    if path.is_dir() {
      copy_dir_all(&path, &dst_path)?;
    } else {
      let _ = fs::copy(&path, &dst_path)?;
    }
  }

  Ok(())
}

/// Show a diff between two files using the `diff` command.
fn show_diff(
  path1: &Path,
  path2: &Path,
) -> Result<()> {
  use std::process::Command;

  let output = Command::new("diff")
    .arg("-u")
    .arg(path1)
    .arg(path2)
    .output()?;

  if let Some(0 | 1) = output.status.code() {
    Ok(())
  } else {
    let stderr = String::from_utf8_lossy(&output.stderr)
      .trim()
      .to_string();
    let stdout = String::from_utf8_lossy(&output.stdout)
      .trim()
      .to_string();
    let message = if !stderr.is_empty() {
      stderr
    } else if !stdout.is_empty() {
      stdout
    } else {
      format!("diff exited with status {}", output.status)
    };
    Err(DotfilesError::Config(format!(
      "Failed to show diff between {} and {}: {}",
      path1.display(),
      path2.display(),
      message
    )))
  }
}
