use crate::config::Config;
use crate::types::{SymlinkResolution, TrackedFile};
use crate::utils::dry_run::{DryRun, Operation};
use crate::utils::error::{DotfilesError, Result};
use crate::utils::path_utils::{
    files_differ, find_symlinked_parent, normalize_path, resolve_symlink_target,
    symlink_points_to_correct_target,
};
use crate::utils::prompt::{ConflictChoice, prompt_conflict_choice};
use crate::utils::security;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, warn};
use std::fs;
use std::path::Path;

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
    // Create backup of destination file if it exists before making any changes
    let home = dirs::home_dir().ok_or_else(crate::utils::error_utils::home_dir_not_found)?;

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
        println!(
            "  [DRY RUN] Would add {} to {} tool",
            source_path.display(),
            tool
        );
    } else {
        config.save(fs_manager.is_dry_run)?;
        // Git-like output: "add 'file'"
        println!("add '{}'", dest_path.display());
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
    verbose: bool,
) -> Result<()> {
    let tracked_files = config.get_tracked_files(profile)?;
    let symlink_resolution = config.general.symlink_resolution;

    if tracked_files.is_empty() {
        println!("{} No files to sync", "⊘".yellow());
        return Ok(());
    }

    // Create progress bar (only show if not verbose and not dry-run)
    let progress_bar = if !verbose && !is_dry_run_mode && tracked_files.len() > 1 {
        let pb = ProgressBar::new(tracked_files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({msg})")
                .map_err(|_| DotfilesError::Config("Failed to create progress bar".to_string()))?
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    if verbose {
        println!("{} Syncing {} file(s)...", "→".cyan(), tracked_files.len());
    }

    // Create FileSystemManager to handle all file operations with dry-run support
    let mut fs_manager = FileSystemManager::new(dry_run_tracker, is_dry_run_mode);

    let mut stats = SyncStats::default();

    // Process files sequentially with progress bar
    for (idx, file) in tracked_files.iter().enumerate() {
        if verbose {
            println!(
                "\n{} [{}/{}] Processing: {}",
                "→".cyan(),
                idx + 1,
                tracked_files.len(),
                file.dest_path.display()
            );
        }

        if let Some(ref pb) = progress_bar {
            pb.set_message(
                file.dest_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string(),
            );
        }

        let result = sync_file(file, symlink_resolution, &mut fs_manager, verbose)?;
        stats.update(result);

        if let Some(ref pb) = progress_bar {
            pb.inc(1);
        }
    }

    // Finish progress bar
    let had_progress_bar = progress_bar.is_some();
    if let Some(pb) = progress_bar {
        pb.finish_with_message("Sync complete");
    }

    // Display sync operation summary
    if verbose {
        println!("\n{} Sync complete", "✓".green());
    } else if !had_progress_bar {
        stats.print_summary();
    }

    Ok(())
}

/// Statistics tracking for file synchronization operations.
#[derive(Default)]
struct SyncStats {
    /// Number of files successfully synced.
    synced: usize,
    /// Number of files skipped during sync.
    skipped: usize,
}

impl SyncStats {
    /// Update statistics based on a sync result.
    const fn update(&mut self, result: SyncResult) {
        match result {
            SyncResult::Synced => self.synced += 1,
            SyncResult::Skipped => self.skipped += 1,
        }
    }

    /// Print a summary of sync statistics.
    fn print_summary(&self) {
        let total = self.synced + self.skipped;
        if total == 0 {
            println!("{} No files to sync", "⊘".yellow());
            return;
        }

        let mut parts = Vec::new();
        if self.synced > 0 {
            parts.push(format!("{} synced", self.synced));
        }
        if self.skipped > 0 {
            parts.push(format!("{} skipped", self.skipped));
        }

        if parts.is_empty() {
            println!("{} Sync complete", "✓".green());
        } else {
            println!("{} Sync complete: {}", "✓".green(), parts.join(", "));
        }
    }
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

        if let Some(entry) = tool_config.files.iter().find(|entry| entry.repo == file) {
            (entry.clone(), false)
        } else if let Some(entry) = tool_config.folders.iter().find(|entry| entry.repo == file) {
            (entry.clone(), true)
        } else {
            return Err(DotfilesError::Path(format!(
                "File {file} not found in tool {tool}"
            )));
        }
    };

    let dest_path = dirs::home_dir()
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
            tool_config.folders.retain(|entry| entry.repo != file);
        } else {
            tool_config.files.retain(|entry| entry.repo != file);
        }

        if tool_config.files.is_empty() && tool_config.folders.is_empty() {
            let _ = config.tools.remove(tool);
        }
    }

    // Save configuration to disk only if not in dry run mode
    if fs_manager.is_dry_run {
        println!("  [DRY RUN] Would remove {file} from {tool} tool");
    } else {
        config.save(fs_manager.is_dry_run)?;
        // Git-like output: "rm 'file'"
        println!("rm '{}'", dest_path.display());
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
    pub const fn new(dry_run: &'a mut DryRun, is_dry_run: bool) -> Self {
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
    pub fn create_dir_all(&mut self, path: &Path) -> Result<()> {
        if self.is_dry_run {
            println!("  [DRY RUN] Would create directory: {}", path.display());
            self.dry_run.log_operation(Operation::CreateDirectory {
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
    pub fn copy(&mut self, from: &Path, to: &Path) -> Result<()> {
        // Safety check: don't copy a file to itself
        if from == to {
            return Err(DotfilesError::Path(format!(
                "Cannot copy file to itself: {}",
                from.display()
            )));
        }

        if self.is_dry_run {
            println!(
                "  [DRY RUN] Would copy file: {} -> {}",
                from.display(),
                to.display()
            );
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
    pub fn copy_dir_all(&mut self, src: &Path, dst: &Path) -> Result<()> {
        // Safety check: don't copy a directory to itself
        if src == dst {
            return Err(DotfilesError::Path(format!(
                "Cannot copy directory to itself: {}",
                src.display()
            )));
        }

        if self.is_dry_run {
            println!(
                "  [DRY RUN] Would copy directory: {} -> {}",
                src.display(),
                dst.display()
            );
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
    pub fn remove_file(&mut self, path: &Path) -> Result<()> {
        if self.is_dry_run {
            let verb = if path.is_dir() && !path.is_symlink() {
                "directory"
            } else {
                "file"
            };
            println!("  [DRY RUN] Would remove {verb}: {}", path.display());
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
    pub fn rename(&mut self, from: &Path, to: &Path) -> Result<()> {
        if self.is_dry_run {
            println!(
                "  [DRY RUN] Would rename: {} -> {}",
                from.display(),
                to.display()
            );
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
    pub fn symlink(&mut self, from: &Path, to: &Path) -> Result<()> {
        if self.is_dry_run {
            println!(
                "  [DRY RUN] Would create symlink: {} -> {}",
                to.display(),
                from.display()
            );
            self.dry_run.log_operation(Operation::CreateSymlink {
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
    verbose: bool,
) -> Result<SyncResult> {
    if verbose {
        println!("  Repo: {}", file.repo_path.display());
        println!("  Dest: {}", file.dest_path.display());
    }

    // Step 1: Precondition checks
    if !file.repo_path.exists() {
        if verbose {
            println!("  {} Repo file does not exist, skipping", "⊘".yellow());
        } else {
            eprintln!(
                "  {} Skipping {} (repo file does not exist)",
                "⊘".yellow(),
                file.dest_path.display()
            );
        }
        return Ok(SyncResult::Skipped);
    }

    // Check if destination file is locked (e.g., in use by another process)
    if let Some(skip_reason) = check_file_lock_status(&file.dest_path, verbose) {
        return Ok(skip_reason);
    }

    // Step 3: Determine what action to take
    let action = determine_sync_action(file, verbose)?;

    // Step 4: Execute the determined action
    match action {
        SyncAction::DoNothing => {
            Ok(SyncResult::Skipped) // Already correctly linked
        }
        SyncAction::CreateSymlink => {
            if verbose {
                println!("  Destination needs to be symlinked.");
            }
            create_symlink_managed(file, resolution, fs_manager, verbose)?;
            Ok(SyncResult::Synced)
        }
        SyncAction::UpdateRepoFromDest => {
            if verbose {
                println!(
                    "{} Repo file {} is empty but destination has content. Updating repo from destination.",
                    "⚠".yellow(),
                    file.repo_path.display()
                );
            } else {
                eprintln!(
                    "  {} Updating {} (repo empty, destination has content)",
                    "⚠".yellow(),
                    file.dest_path.display()
                );
            }
            if let Some(parent) = file.repo_path.parent() {
                fs_manager.create_dir_all(parent)?;
            }
            if file.dest_path.is_dir() {
                fs_manager.copy_dir_all(&file.dest_path, &file.repo_path)?;
            } else {
                fs_manager.copy(&file.dest_path, &file.repo_path)?;
            }

            if verbose && !fs_manager.is_dry_run {
                println!("{} Updated repo file from destination", "✓".green());
            }
            // Now that repo is updated, create the symlink
            create_symlink_managed(file, resolution, fs_manager, verbose)?;
            Ok(SyncResult::Synced)
        }
        SyncAction::ResolveConflict => {
            handle_file_conflict(file, resolution, fs_manager, verbose)?;
            Ok(SyncResult::Synced)
        }
    }
}

/// Determines what action to take for a file. (No side-effects)
fn determine_sync_action(file: &TrackedFile, verbose: bool) -> Result<SyncAction> {
    if !file.dest_path.exists() && !file.dest_path.is_symlink() {
        if verbose {
            println!("  Destination does not exist");
        }
        return Ok(SyncAction::CreateSymlink);
    }

    if verbose {
        println!("  Destination exists");
    }

    // Check if it's a symlink and already correctly linked
    if file.dest_path.is_symlink()
        && let Ok(link_target) = fs::read_link(&file.dest_path)
    {
        if verbose {
            println!("  Is symlink pointing to: {}", link_target.display());
        }
        if symlink_points_to_correct_target(&file.dest_path, &link_target, &file.repo_path) {
            if verbose {
                println!("  {} Already correctly linked", "✓".green());
            }
            return Ok(SyncAction::DoNothing);
        }
        if verbose {
            println!("  {} Symlink points to wrong location", "⚠".yellow());
        }
        return Ok(SyncAction::ResolveConflict);
    }

    if verbose {
        println!("  Is regular file/directory (not symlink)");
    }

    // Check if a parent directory is symlinked - if so, we need to convert it
    if let Some((symlinked_parent, resolved_parent)) = find_symlinked_parent(&file.dest_path) {
        if verbose {
            println!(
                "  {} Parent directory {} is symlinked to {}",
                "⚠".yellow(),
                symlinked_parent.display(),
                resolved_parent.display()
            );
        }
        // Get the relative path from the symlinked parent to the file
        if let Ok(relative) = file.dest_path.strip_prefix(&symlinked_parent) {
            let resolved_file_path = resolved_parent.join(relative);
            let normalized_resolved = normalize_path(&resolved_file_path);
            let normalized_repo = normalize_path(&file.repo_path);

            // If the resolved path through the symlinked parent matches the repo path,
            // we need to convert the directory symlink to individual file symlinks
            if normalized_resolved == normalized_repo {
                if verbose {
                    println!(
                        "  File is effectively synced via directory symlink, will convert to individual symlink"
                    );
                }
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
        if verbose {
            println!(
                "  {} Safety check: Repo is empty, destination has content",
                "⚠".yellow()
            );
        }
        return Ok(SyncAction::UpdateRepoFromDest);
    }

    // Check if files are different
    if verbose {
        println!("  Comparing files...");
    }
    if files_differ(&file.repo_path, &file.dest_path)? {
        if verbose {
            println!("  {} Files differ", "↻".yellow());
        }
        Ok(SyncAction::ResolveConflict)
    } else {
        if verbose {
            println!("  {} Files are identical", "✓".green());
        }
        // Files are identical, but dest is not a symlink. Convert it.
        Ok(SyncAction::CreateSymlink)
    }
}

/// Handles the user-interactive part of resolving a file conflict.
/// Assumes backup has already been created.
fn handle_file_conflict(
    file: &TrackedFile,
    resolution: SymlinkResolution,
    fs_manager: &mut FileSystemManager,
    verbose: bool,
) -> Result<()> {
    let conflict_choice = if fs_manager.is_dry_run {
        if verbose {
            println!("  [DRY RUN] Files differ, would prompt for conflict resolution");
            println!("  [DRY RUN] Assuming: Backup and Replace");
        }
        ConflictChoice::BackupAndUseRepo
    } else {
        // Backup was already created in sync_file
        prompt_conflict_choice(&file.dest_path)?
    };

    match conflict_choice {
        ConflictChoice::BackupAndUseRepo | ConflictChoice::UseRepo => {
            if verbose {
                println!("  User chose: Backup and Replace");
            }
            create_symlink_managed(file, resolution, fs_manager, verbose)?;
        }
        ConflictChoice::KeepLocal => {
            if verbose {
                println!("  {} User chose: Keep Local", "⊘".yellow());
            }
            return Ok(());
        }
        ConflictChoice::ViewDiff => {
            if fs_manager.is_dry_run {
                if verbose {
                    println!("  [DRY RUN] Would show diff and prompt again");
                    println!("  [DRY RUN] Assuming: Backup and Replace");
                }
                create_symlink_managed(file, resolution, fs_manager, verbose)?;
            } else {
                show_diff(&file.repo_path, &file.dest_path)?;
                // Ask again after showing diff
                let post_diff_choice = prompt_conflict_choice(&file.dest_path)?;
                match post_diff_choice {
                    ConflictChoice::BackupAndUseRepo | ConflictChoice::UseRepo => {
                        create_symlink_managed(file, resolution, fs_manager, verbose)?;
                    }
                    ConflictChoice::KeepLocal => {
                        if verbose {
                            println!("{} Skipped {}", "⊘".yellow(), file.dest_path.display());
                        }
                    }
                    ConflictChoice::Abort => {
                        return Err(DotfilesError::Cancelled);
                    }
                    ConflictChoice::ViewDiff => {} // ViewDiff again is not an option here
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
/// Assumes backups have *already been created* by the caller.
fn create_symlink_managed(
    file: &TrackedFile,
    resolution: SymlinkResolution,
    fs_manager: &mut FileSystemManager,
    verbose: bool,
) -> Result<()> {
    validate_symlink_security(file)?;

    debug!(
        "Creating symlink from {} to {}",
        file.dest_path.display(),
        file.repo_path.display()
    );

    ensure_parent_directory(file, fs_manager, verbose)?;

    // Handle "Replace" strategy: copy file instead of creating symlink
    if resolution == SymlinkResolution::Replace {
        handle_replace_strategy(file, fs_manager, verbose)?;
        return Ok(());
    }

    // Handle regular symlink creation (non-Replace strategies)
    let link_target = compute_symlink_target(file, resolution, verbose)?;
    create_symlink_atomic(file, &link_target, fs_manager, verbose)?;

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
    verbose: bool,
) -> Result<()> {
    if let Some(parent) = file.dest_path.parent() {
        // Check if parent is a symlink pointing to repo - if so, warn but continue
        if let Ok(link_target) = fs::read_link(parent) {
            let resolved_target = resolve_symlink_target(parent, &link_target);
            let normalized_target = normalize_path(&resolved_target);
            if let Some(repo_parent) = file.repo_path.parent() {
                let normalized_repo_parent = normalize_path(repo_parent);
                if normalized_target == normalized_repo_parent && verbose {
                    println!(
                        "    {} Warning: Parent directory {} is symlinked to repo. ",
                        "⚠".yellow(),
                        parent.display()
                    );
                    println!(
                        "    To convert to individual file symlinks, remove the directory symlink first."
                    );
                }
            }
        }
        fs_manager.create_dir_all(parent)?;
    }
    Ok(())
}

/// Handles the Replace strategy by copying files instead of creating symlinks.
fn handle_replace_strategy(
    file: &TrackedFile,
    fs_manager: &mut FileSystemManager,
    verbose: bool,
) -> Result<()> {
    let temp_path = file.dest_path.with_extension("flux-temp-copy");
    if verbose {
        println!("    Copying file instead of symlinking (Replace strategy)...");
    }

    fs_manager.copy(&file.repo_path, &temp_path)?;
    fs_manager.rename(&temp_path, &file.dest_path)?; // Atomic move

    if verbose && !fs_manager.is_dry_run {
        println!(
            "{} Copied {} -> {}",
            "✓".green(),
            file.repo_path.display(),
            file.dest_path.display()
        );
    }
    Ok(())
}

/// Computes the symlink target path based on the resolution strategy.
fn compute_symlink_target(
    file: &TrackedFile,
    resolution: SymlinkResolution,
    verbose: bool,
) -> Result<std::path::PathBuf> {
    let link_target = match resolution {
        SymlinkResolution::Auto => file.dest_path.parent().map_or_else(
            || file.repo_path.clone(),
            |parent| {
                pathdiff::diff_paths(&file.repo_path, parent)
                    .unwrap_or_else(|| file.repo_path.clone())
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
        SymlinkResolution::Follow => {
            if verbose {
                println!("    'Follow' resolution strategy is treated as 'Auto'.");
            }
            file.dest_path.parent().map_or_else(
                || file.repo_path.clone(),
                |parent| {
                    pathdiff::diff_paths(&file.repo_path, parent)
                        .unwrap_or_else(|| file.repo_path.clone())
                },
            )
        }
        SymlinkResolution::Replace => unreachable!(), // Handled earlier
    };
    Ok(link_target)
}

/// Creates a symlink atomically using a temporary path.
fn create_symlink_atomic(
    file: &TrackedFile,
    link_target: &Path,
    fs_manager: &mut FileSystemManager,
    verbose: bool,
) -> Result<()> {
    let ext_str = file.dest_path.extension().and_then(|ext| ext.to_str()).unwrap_or_default();
    let temp_link_path = file.dest_path.with_extension(format!("{ext_str}.flux-temp"));

    if verbose {
        println!(
            "    Creating temp symlink: {} -> {}",
            temp_link_path.display(),
            link_target.display()
        );
    }
    // Remove any existing temporary symlink from previous failed operations
    let _ = fs_manager.remove_file(&temp_link_path);
    fs_manager.symlink(link_target, &temp_link_path)?;

    // Atomically rename temporary symlink to final destination
    if verbose {
        println!(
            "    Atomically moving link: {} -> {}",
            temp_link_path.display(),
            file.dest_path.display()
        );
    }
    fs_manager.rename(&temp_link_path, &file.dest_path)?;

    if verbose && !fs_manager.is_dry_run {
        println!(
            "    {} Linked {} -> {}",
            "✓".green(),
            file.repo_path.display(),
            file.dest_path.display()
        );
    }

    Ok(())
}

// #############################################################################
// ## Module-Private Helpers
// #############################################################################

/// Check if a file is locked and handle the result.
///
/// Returns `Some(SyncResult::Skipped)` if the file is locked and should be skipped,
/// or `None` if the file is not locked or lock check failed (continues processing).
fn check_file_lock_status(file_path: &Path, verbose: bool) -> Option<SyncResult> {
    if !file_path.exists() {
        return None;
    }

    match security::is_file_locked(file_path) {
        Ok(true) => {
            warn!(
                "File {} is locked (may be in use), skipping",
                file_path.display()
            );
            if verbose {
                println!(
                    "  {} File is locked (may be in use by another application), skipping",
                    "⚠".yellow()
                );
            } else {
                eprintln!(
                    "  {} Skipping {} (file is locked)",
                    "⚠".yellow(),
                    file_path.display()
                );
            }
            Some(SyncResult::Skipped)
        }
        Ok(false) => {
            debug!("File {} is not locked", file_path.display());
            None
        }
        Err(error) => {
            warn!(
                "Could not check lock status for {}: {}",
                file_path.display(),
                error
            );
            debug!("Continuing despite lock check error");
            None
        }
    }
}

/// Recursively copy a directory.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
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
fn show_diff(path1: &Path, path2: &Path) -> Result<()> {
    use std::process::Command;

    let output = Command::new("diff").arg("-u").arg(path1).arg(path2).output()?;

    // `diff` returns 1 if files differ, 0 if same, 2 on error.
    // We want to print stdout for both 0 and 1.
    if output.status.success() || output.status.code() == Some(1) {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        // Print stderr if `diff` command failed
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}
