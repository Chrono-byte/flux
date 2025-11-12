use crate::config::Config;
use crate::types::{SymlinkResolution, TrackedFile};
use crate::utils::dry_run::{DryRun, Operation};
use crate::utils::error::{DotfilesError, Result};
use crate::utils::prompt::{ConflictResolution, prompt_conflict};
use crate::utils::security;
use chrono::Local;
use colored::Colorize;
use log::{debug, warn};
use std::fs;
use std::path::{Path, PathBuf};

// #############################################################################
// ## Public API Functions
// #############################################################################

/// Add a file to the dotfiles repository.
pub fn add_file(
    config: &mut Config,
    tool: &str,
    source_path: &Path,
    dest_path: &Path,
    profile: Option<&str>,
    fs_manager: &mut FileSystemManager,
) -> Result<()> {
    // BACKUP: Create backup of destination file if it exists BEFORE any changes
    let home = dirs::home_dir().ok_or_else(crate::utils::error_utils::home_dir_not_found)?;

    // Validate destination path is within home directory
    crate::utils::security::validate_dest_path(dest_path, &home)?;

    let full_dest_path = home.join(dest_path);
    if let Some(path_to_backup) = get_path_to_backup(&full_dest_path) {
        println!("  Creating backup of existing destination...");
        fs_manager.backup_file(&path_to_backup, config, None)?;
    }

    let repo_path = config.get_repo_path()?;
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

    // Copy file or directory to repo
    fs_manager.create_dir_all(&tool_dir)?;

    if source_path.is_dir() {
        fs_manager.copy_dir_all(source_path, &repo_file)?;
    } else {
        fs_manager.copy(source_path, &repo_file)?;
    }

    // Add to config (in memory)
    let repo_relative = repo_file
        .strip_prefix(&repo_path)
        .map_err(|e| {
            crate::utils::error_utils::invalid_path_computation(
                &repo_path,
                &repo_file,
                &format!("Repository file is not within repository directory: {}", e),
            )
        })?
        .to_string_lossy()
        .to_string();
    config.add_file_to_tool(tool, &repo_relative, dest_path, profile)?;

    // Only save config if not in dry run mode
    if !fs_manager.is_dry_run {
        config.save(fs_manager.is_dry_run)?;
        println!(
            "{} Added {} to {} tool",
            "✓".green(),
            source_path.display(),
            tool
        );
    } else {
        println!(
            "  [DRY RUN] Would add {} to {} tool",
            source_path.display(),
            tool
        );
    }

    Ok(())
}

/// Sync all tracked files, creating symlinks from repo to destination.
pub fn sync_files(
    config: &Config,
    profile: Option<&str>,
    dry_run_tracker: &mut DryRun,
    is_dry_run_mode: bool,
    verbose: bool,
) -> Result<()> {
    let tracked_files = config.get_tracked_files(profile)?;
    let symlink_resolution = config.general.symlink_resolution;

    // Create the FileSystemManager here. It will be passed down.
    let mut fs_manager = FileSystemManager::new(dry_run_tracker, is_dry_run_mode);

    // Create a single timestamped backup directory for all files in this sync operation
    let backup_dir = config
        .get_backup_dir()?
        .join(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string());

    if verbose {
        println!("{} Syncing {} file(s)...", "→".cyan(), tracked_files.len());
    }

    let mut stats = SyncStats::default();

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
        let result = sync_file(
            file,
            &symlink_resolution,
            config,
            &mut fs_manager,
            Some(&backup_dir),
            verbose,
        )?;
        stats.update(result);
    }

    // Print summary
    if verbose {
        println!("\n{} Sync complete", "✓".green());
    } else {
        stats.print_summary();
    }
    Ok(())
}

#[derive(Default)]
struct SyncStats {
    synced: usize,
    skipped: usize,
}

impl SyncStats {
    fn update(&mut self, result: SyncResult) {
        match result {
            SyncResult::Synced => self.synced += 1,
            SyncResult::Skipped => self.skipped += 1,
        }
    }

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

#[derive(Clone, Copy)]
enum SyncResult {
    Synced,
    Skipped,
}

/// Backup all currently tracked files.
pub fn backup_all_files(
    config: &Config,
    profile: Option<&str>,
    dry_run_tracker: &mut DryRun,
    is_dry_run_mode: bool,
) -> Result<()> {
    let tracked_files = config.get_tracked_files(profile)?;

    if tracked_files.is_empty() {
        println!("{} No tracked files to backup.", "⊘".yellow());
        return Ok(());
    }

    println!(
        "{} Backing up {} tracked file(s)...",
        "→".cyan(),
        tracked_files.len()
    );

    // Create a manager for this operation
    let mut fs_manager = FileSystemManager::new(dry_run_tracker, is_dry_run_mode);

    if is_dry_run_mode {
        println!(
            "{} DRY RUN MODE - No files will be modified",
            "⚠".yellow().bold()
        );
    }

    // Create a single timestamped backup directory for all files
    let backup_dir = config
        .get_backup_dir()?
        .join(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string());
    let canonical_backup_dir = normalize_path(&backup_dir);

    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?;
    let canonical_home = normalize_path(&home);

    let mut backed_up_count = 0;
    let mut skipped_count = 0;

    for file in &tracked_files {
        // Use the centralized helper to find what to back up
        let file_to_backup = match get_path_to_backup(&file.dest_path) {
            Some(path) => path,
            None => {
                println!(
                    "  {} Skipping {} (destination does not exist or is broken symlink)",
                    "⊘".yellow(),
                    file.dest_path.display()
                );
                skipped_count += 1;
                continue;
            }
        };

        // SAFETY CHECK: Ensure source is not inside backup directory
        let canonical_source = normalize_path(&file_to_backup);
        if canonical_source.starts_with(&canonical_backup_dir) {
            println!(
                "  {} Skipping {} (source is inside backup directory)",
                "⊘".yellow(),
                file_to_backup.display()
            );
            skipped_count += 1;
            continue;
        }

        // Calculate backup path
        let relative_path = canonical_source
            .strip_prefix(&canonical_home)
            .unwrap_or(&canonical_source);
        let backup_path = backup_dir.join(relative_path);

        // Create parent directory
        if let Some(parent) = backup_path.parent() {
            fs_manager.create_dir_all(parent)?;
        }

        // Copy file or directory using the manager
        if file_to_backup.is_dir() {
            fs_manager.copy_dir_all(&file_to_backup, &backup_path)?;
        } else {
            fs_manager.copy(&file_to_backup, &backup_path)?;
        }

        // Log progress (fs_manager already logged the dry-run op)
        if !is_dry_run_mode {
            println!(
                "  {} Backed up {} -> {}",
                "✓".yellow(),
                file_to_backup.display(),
                backup_path.display()
            );
        }
        backed_up_count += 1;
    }

    println!("\n{} Backup complete", "✓".green());
    println!(
        "  {} backed up, {} skipped",
        backed_up_count.to_string().green(),
        skipped_count.to_string().yellow()
    );

    if !is_dry_run_mode && backed_up_count > 0 {
        println!(
            "  Backup location: {}",
            backup_dir.display().to_string().cyan()
        );
    }

    Ok(())
}

/// Remove a file from tracking and delete it from the filesystem.
pub fn remove_file(
    config: &mut Config,
    tool: &str,
    file: &str,
    fs_manager: &mut FileSystemManager,
) -> Result<()> {
    // Find the file in config
    let tool_config = config
        .tools
        .get(tool)
        .ok_or_else(|| DotfilesError::InvalidTool(tool.to_string()))?;

    let file_entry = tool_config
        .files
        .iter()
        .find(|e| e.repo == file)
        .ok_or_else(|| DotfilesError::Path(format!("File {} not found in tool {}", file, tool)))?;

    let dest_path = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?
        .join(&file_entry.dest);

    // BACKUP: Create backup before removing
    if let Some(path_to_backup) = get_path_to_backup(&dest_path) {
        println!("  Creating backup before removal...");
        fs_manager.backup_file(&path_to_backup, config, None)?;
    }

    // Remove symlink/file from destination
    fs_manager.remove_file(&dest_path)?;

    // Remove from repo
    let repo_path = config.get_repo_path()?;
    // Handle both cases: file may or may not include the tool name prefix
    let repo_file = if file.starts_with(&format!("{}/", tool)) {
        // file already includes tool name (e.g., "cursor/settings.json")
        repo_path.join(file)
    } else {
        // file doesn't include tool name (e.g., "config")
        repo_path.join(tool).join(file)
    };
    fs_manager.remove_file(&repo_file)?;

    // Remove from config (in memory)
    if let Some(tool_config) = config.tools.get_mut(tool) {
        tool_config.files.retain(|e| e.repo != file);
        if tool_config.files.is_empty() {
            config.tools.remove(tool);
        }
    }

    // Only save config if not in dry run mode
    if !fs_manager.is_dry_run {
        config.save(fs_manager.is_dry_run)?;
        println!("{} Removed {} from {} tool", "✓".green(), file, tool);
    } else {
        println!("  [DRY RUN] Would remove {} from {} tool", file, tool);
    }

    Ok(())
}

// #############################################################################
// ## FileSystemManager Struct
// #############################################################################

/// Manages all file system operations, respecting dry run mode.
/// This struct abstracts all file I/O, allowing other functions
/// to focus on logic rather than implementation details.
pub struct FileSystemManager<'a> {
    dry_run: &'a mut DryRun,
    pub is_dry_run: bool,
}

impl<'a> FileSystemManager<'a> {
    /// Create a new FileSystemManager.
    pub fn new(dry_run: &'a mut DryRun, is_dry_run: bool) -> Self {
        Self {
            dry_run,
            is_dry_run,
        }
    }

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

    pub fn remove_file(&mut self, path: &Path) -> Result<()> {
        if self.is_dry_run {
            println!("  [DRY RUN] Would remove file: {}", path.display());
            self.dry_run.log_operation(Operation::RemoveFile {
                path: path.to_path_buf(),
            });
            Ok(())
        } else {
            // Check if it exists (as file) or is a symlink (which might be broken)
            if path.exists() || path.is_symlink() {
                fs::remove_file(path).map_err(Into::into)
            } else {
                Ok(()) // Already gone
            }
        }
    }

    pub fn rename(&mut self, from: &Path, to: &Path) -> Result<()> {
        if self.is_dry_run {
            println!(
                "  [DRY RUN] Would rename: {} -> {}",
                from.display(),
                to.display()
            );
            self.dry_run.log_operation(Operation::CopyFile {
                // You may want to add a Rename operation
                from: from.to_path_buf(),
                to: to.to_path_buf(),
            });
            Ok(())
        } else {
            fs::rename(from, to).map_err(Into::into)
        }
    }

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
            // START: Cross-platform symlink logic
            #[cfg(unix)]
            {
                use std::os::unix::fs::symlink;
                symlink(from, to).map_err(Into::into)
            }
            #[cfg(windows)]
            {
                // Windows requires knowing if the target is a file or directory
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
            // END: Cross-platform symlink logic
        }
    }

    /// Creates a backup of a file using the backup directory from config.
    /// If `backup_dir` is provided, uses that directory; otherwise creates a new timestamped directory.
    pub fn backup_file(
        &mut self,
        file_path: &Path,
        config: &Config,
        backup_dir: Option<&Path>,
    ) -> Result<PathBuf> {
        let backup_dir = if let Some(dir) = backup_dir {
            dir.to_path_buf()
        } else {
            config
                .get_backup_dir()?
                .join(Local::now().format("%Y%m%d_%H%M%S").to_string())
        };

        let home = dirs::home_dir()
            .ok_or_else(|| DotfilesError::Path("Could not find home directory".to_string()))?;
        let relative_path = file_path.strip_prefix(&home).unwrap_or(file_path);
        let backup_path = backup_dir.join(relative_path);

        if self.is_dry_run {
            println!(
                "  [DRY RUN] Would backup {} -> {}",
                file_path.display(),
                backup_path.display()
            );
            self.dry_run.log_operation(Operation::CreateBackup {
                file: file_path.to_path_buf(),
                backup: backup_path.clone(),
            });
        } else {
            fs::create_dir_all(backup_path.parent().unwrap())?;

            if file_path.is_dir() {
                copy_dir_all(file_path, &backup_path)?;
            } else {
                fs::copy(file_path, &backup_path)?;
            }

            // SECURITY: Set secure permissions on backup files (0600 - owner only)
            if let Err(e) = security::set_secure_permissions(&backup_path) {
                warn!(
                    "Could not set secure permissions on backup {}: {}",
                    backup_path.display(),
                    e
                );
            }

            println!(
                "{} Backed up {} -> {}",
                "✓".yellow(),
                file_path.display(),
                backup_path.display()
            );
        }

        Ok(backup_path)
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
    resolution: &SymlinkResolution,
    config: &Config,
    fs_manager: &mut FileSystemManager,
    backup_dir: Option<&Path>,
    verbose: bool,
) -> Result<SyncResult> {
    if verbose {
        println!("  Repo: {}", file.repo_path.display());
        println!("  Dest: {}", file.dest_path.display());
    }

    // --- 1. Precondition Checks ---
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
    if file.dest_path.exists() {
        match security::is_file_locked(&file.dest_path) {
            Ok(true) => {
                warn!(
                    "File {} is locked (may be in use), skipping",
                    file.dest_path.display()
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
                        file.dest_path.display()
                    );
                }
                return Ok(SyncResult::Skipped);
            }
            Ok(false) => {
                debug!("File {} is not locked", file.dest_path.display());
            }
            Err(e) => {
                warn!(
                    "Could not check lock status for {}: {}",
                    file.dest_path.display(),
                    e
                );
                debug!("Continuing despite lock check error");
            }
        }
    }

    // --- 2. Backup ---
    // Backup *before* determining action, as any action (except DoNothing)
    // might modify the destination. This simplifies all downstream logic.
    if let Some(path_to_backup) = get_path_to_backup(&file.dest_path) {
        if verbose {
            println!("  Creating backup before any modifications...");
        }
        fs_manager.backup_file(&path_to_backup, config, backup_dir)?;
    }

    // --- 3. Determine Action ---
    let action = determine_sync_action(file, verbose)?;

    // --- 4. Execute Action ---
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

        if normalized_target == normalized_repo {
            if verbose {
                println!("  {} Already correctly linked", "✓".green());
            }
            return Ok(SyncAction::DoNothing);
        } else {
            if verbose {
                println!("  {} Symlink points to wrong location", "⚠".yellow());
            }
            return Ok(SyncAction::ResolveConflict);
        }
    }

    if verbose {
        println!("  Is regular file/directory (not symlink)");
    }

    // SAFETY CHECK: Don't overwrite non-empty destination with empty repo file
    let repo_is_empty = if file.repo_path.is_file() {
        fs::metadata(&file.repo_path).map(|m| m.len()).unwrap_or(0) == 0
    } else {
        false // Dirs aren't "empty" for this check
    };
    let dest_has_content = if file.dest_path.is_file() {
        fs::metadata(&file.dest_path).map(|m| m.len()).unwrap_or(0) > 0
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
    resolution: &SymlinkResolution,
    fs_manager: &mut FileSystemManager,
    verbose: bool,
) -> Result<()> {
    let conflict_resolution = if fs_manager.is_dry_run {
        if verbose {
            println!("  [DRY RUN] Files differ, would prompt for conflict resolution");
            println!("  [DRY RUN] Assuming: Backup and Replace");
        }
        ConflictResolution::BackupAndReplace
    } else {
        // Backup was already created in sync_file
        prompt_conflict(&file.dest_path)?
    };

    match conflict_resolution {
        ConflictResolution::BackupAndReplace => {
            if verbose {
                println!("  User chose: Backup and Replace");
            }
            create_symlink_managed(file, resolution, fs_manager, verbose)?;
        }
        ConflictResolution::Skip => {
            if verbose {
                println!("  {} User chose: Skip", "⊘".yellow());
            }
            return Ok(());
        }
        ConflictResolution::ViewDiff => {
            if !fs_manager.is_dry_run {
                show_diff(&file.repo_path, &file.dest_path)?;
                // Ask again after showing diff
                let post_diff_resolution = prompt_conflict(&file.dest_path)?;
                match post_diff_resolution {
                    ConflictResolution::BackupAndReplace => {
                        create_symlink_managed(file, resolution, fs_manager, verbose)?;
                    }
                    ConflictResolution::Skip => {
                        if verbose {
                            println!("{} Skipped {}", "⊘".yellow(), file.dest_path.display());
                        }
                    }
                    ConflictResolution::Cancel => {
                        return Err(DotfilesError::Cancelled);
                    }
                    _ => {} // ViewDiff again is not an option here
                }
            } else {
                if verbose {
                    println!("  [DRY RUN] Would show diff and prompt again");
                    println!("  [DRY RUN] Assuming: Backup and Replace");
                }
                create_symlink_managed(file, resolution, fs_manager, verbose)?;
            }
        }
        ConflictResolution::Cancel => {
            return Err(DotfilesError::Cancelled);
        }
    }
    Ok(())
}

/// Creates a symlink, managed by the FileSystemManager.
/// Assumes backups have *already been created* by the caller.
fn create_symlink_managed(
    file: &TrackedFile,
    resolution: &SymlinkResolution,
    fs_manager: &mut FileSystemManager,
    verbose: bool,
) -> Result<()> {
    // SECURITY: Validate symlink target is within repo
    if let Err(e) = security::validate_symlink_target(&file.repo_path, &file.repo_path) {
        warn!(
            "Symlink validation failed for {}: {}",
            file.repo_path.display(),
            e
        );
        return Err(e);
    }

    debug!(
        "Creating symlink from {} to {}",
        file.dest_path.display(),
        file.repo_path.display()
    );

    // 1. Create parent directory if needed
    if let Some(parent) = file.dest_path.parent() {
        fs_manager.create_dir_all(parent)?;
    }

    // 3. Handle the "Replace" (copy) case
    if *resolution == SymlinkResolution::Replace {
        // ... (This logic is OK, but we must use atomic rename)
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
        return Ok(());
    }

    // 4. Handle regular symlinking
    let link_target = match resolution {
        // ... (this logic is fine)
        SymlinkResolution::Auto => {
            pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                .unwrap_or_else(|| file.repo_path.clone())
        }
        SymlinkResolution::Relative => {
            pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                .ok_or_else(|| DotfilesError::Path("Cannot create relative symlink".to_string()))?
        }
        SymlinkResolution::Absolute => file.repo_path.clone(),
        SymlinkResolution::Follow => {
            if verbose {
                println!("    'Follow' resolution strategy is treated as 'Auto'.");
            }
            pathdiff::diff_paths(&file.repo_path, file.dest_path.parent().unwrap())
                .unwrap_or_else(|| file.repo_path.clone())
        }
        SymlinkResolution::Replace => unreachable!(), // Handled above
    };

    // 5. NEW ATOMIC SYMLINK LOGIC
    // Create the symlink at a temporary path
    let temp_link_path = file.dest_path.with_extension(format!(
        "{}.flux-temp",
        file.dest_path
            .extension()
            .map_or("", |s| s.to_str().unwrap_or(""))
    ));

    if verbose {
        println!(
            "    Creating temp symlink: {} -> {}",
            temp_link_path.display(),
            link_target.display()
        );
    }
    // Ensure old temp link is gone (in case of failed previous run)
    let _ = fs_manager.remove_file(&temp_link_path);
    fs_manager.symlink(&link_target, &temp_link_path)?;

    // Atomically rename the temp symlink to the final destination
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

/// Normalize a path by canonicalizing it, falling back to the path itself if canonicalization fails
fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Resolves the actual file to be backed up.
/// If `path` is a file/dir, returns `Some(path)`.
/// If `path` is a symlink, returns its *target* path.
/// If `path` doesn't exist or is a broken symlink, returns `None`.
fn get_path_to_backup(path: &Path) -> Option<PathBuf> {
    if path.is_symlink() {
        match fs::read_link(path) {
            Ok(target) => {
                let resolved_target = if target.is_absolute() {
                    target
                } else {
                    path.parent().map(|p| p.join(&target)).unwrap_or(target)
                };
                let normalized_target = normalize_path(&resolved_target);
                if normalized_target.exists() {
                    Some(normalized_target)
                } else {
                    None // Broken symlink
                }
            }
            Err(_) => None, // Can't read symlink
        }
    } else if path.exists() {
        Some(path.to_path_buf()) // Regular file or directory
    } else {
        None // Doesn't exist
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
            fs::copy(&path, &dst_path)?;
        }
    }

    Ok(())
}

/// Check if two files have different content.
fn files_differ(path1: &Path, path2: &Path) -> Result<bool> {
    if !path1.exists() || !path2.exists() {
        return Ok(true); // One or both don't exist, so they are "different"
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

/// Show a diff between two files using the `diff` command.
fn show_diff(path1: &Path, path2: &Path) -> Result<()> {
    use std::process::Command;

    let output = Command::new("diff")
        .arg("-u")
        .arg(path1)
        .arg(path2)
        .output()?;

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
