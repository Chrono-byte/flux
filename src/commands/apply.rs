use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::services::{
    FileOperation, PackageManagerType, ServiceManager, SystemdServiceManager, Transaction,
};
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
    /// Use sudo for operations requiring elevated privileges
    pub use_sudo: bool,
    /// Use user services instead of system services
    pub user_services: bool,
    /// Optional description for this apply operation
    pub description: Option<&'a str>,
    /// Package manager type to use
    pub package_manager_type: PackageManagerType,
}

/// State comparison result showing what needs to change
#[derive(Debug, Clone)]
pub struct StateDiff {
    /// Files that need to be synced
    pub files_to_sync: Vec<TrackedFile>,
    /// Packages that need to be installed
    pub packages_to_install: Vec<(String, String)>,
    /// Packages that need to be removed
    pub packages_to_remove: Vec<String>,
    /// Services that need to be enabled
    pub services_to_enable: Vec<(String, bool)>,
    /// Services that need to be disabled
    pub services_to_disable: Vec<(String, bool)>,
    /// Services that need to be started
    pub services_to_start: Vec<(String, bool)>,
    /// Services that need to be stopped
    pub services_to_stop: Vec<(String, bool)>,
}

impl StateDiff {
    pub fn new() -> Self {
        Self {
            files_to_sync: Vec::new(),
            packages_to_install: Vec::new(),
            packages_to_remove: Vec::new(),
            services_to_enable: Vec::new(),
            services_to_disable: Vec::new(),
            services_to_start: Vec::new(),
            services_to_stop: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files_to_sync.is_empty()
            && self.packages_to_install.is_empty()
            && self.packages_to_remove.is_empty()
            && self.services_to_enable.is_empty()
            && self.services_to_disable.is_empty()
            && self.services_to_start.is_empty()
            && self.services_to_stop.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.files_to_sync.len()
            + self.packages_to_install.len()
            + self.packages_to_remove.len()
            + self.services_to_enable.len()
            + self.services_to_disable.len()
            + self.services_to_start.len()
            + self.services_to_stop.len()
    }
}

/// Compare declared state (from config) with actual system state
pub fn compare_states(
    config: &Config,
    profile: Option<&str>,
    use_sudo: bool,
    user_services: bool,
    package_manager_type: PackageManagerType,
) -> Result<StateDiff> {
    let mut diff = StateDiff::new();

    // Compare files
    let tracked_files = config.get_tracked_files(profile)?;
    for file in tracked_files {
        if needs_sync(&file)? {
            diff.files_to_sync.push(file);
        }
    }

    // Compare packages
    let package_manager = package_manager_type.create_manager(use_sudo);
    for (name, spec) in &config.packages {
        let package_name = spec.name.as_ref().unwrap_or(name);
        match package_manager.is_installed(package_name) {
            Ok(false) => {
                diff.packages_to_install
                    .push((package_name.clone(), spec.version.clone()));
            }
            Ok(true) => {
                // Check version if specified
                if spec.version != "latest"
                    && let Ok(Some(installed_version)) = package_manager.get_version(package_name)
                {
                    // Simple version comparison (could be enhanced)
                    if installed_version != spec.version {
                        diff.packages_to_install
                            .push((package_name.clone(), spec.version.clone()));
                    }
                }
            }
            Err(_) => {
                // Package manager unavailable, skip
            }
        }
    }

    // Compare services
    let service_manager = SystemdServiceManager::new(user_services);
    for (name, spec) in &config.services {
        let service_name = spec.name.as_ref().unwrap_or(name);

        // Check enabled state
        match service_manager.is_enabled(service_name) {
            Ok(enabled) => {
                if spec.enabled && !enabled {
                    diff.services_to_enable
                        .push((service_name.clone(), !user_services));
                } else if !spec.enabled && enabled {
                    diff.services_to_disable
                        .push((service_name.clone(), !user_services));
                }
            }
            Err(_) => {
                // Service manager unavailable, skip
            }
        }

        // Check running state
        if let Some(should_run) = spec.running {
            match service_manager.is_running(service_name) {
                Ok(running) => {
                    if should_run && !running {
                        diff.services_to_start
                            .push((service_name.clone(), !user_services));
                    } else if !should_run && running {
                        diff.services_to_stop
                            .push((service_name.clone(), !user_services));
                    }
                }
                Err(_) => {
                    // Service manager unavailable, skip
                }
            }
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

    if diff.files_to_sync.is_empty()
        && diff.packages_to_install.is_empty()
        && diff.packages_to_remove.is_empty()
        && diff.services_to_enable.is_empty()
        && diff.services_to_disable.is_empty()
        && diff.services_to_start.is_empty()
        && diff.services_to_stop.is_empty()
    {
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

    if !diff.packages_to_install.is_empty() {
        println!(
            "\n  {} Packages to install ({}):",
            "📦".cyan(),
            diff.packages_to_install.len()
        );
        for (name, version) in &diff.packages_to_install {
            println!("    • {} ({})", name, version);
        }
    }

    if !diff.packages_to_remove.is_empty() {
        println!(
            "\n  {} Packages to remove ({}):",
            "🗑️".cyan(),
            diff.packages_to_remove.len()
        );
        for name in &diff.packages_to_remove {
            println!("    • {}", name);
        }
    }

    if !diff.services_to_enable.is_empty() {
        println!(
            "\n  {} Services to enable ({}):",
            "▶".cyan(),
            diff.services_to_enable.len()
        );
        for (name, system) in &diff.services_to_enable {
            let scope = if *system { "system" } else { "user" };
            println!("    • {} ({})", name, scope);
        }
    }

    if !diff.services_to_disable.is_empty() {
        println!(
            "\n  {} Services to disable ({}):",
            "⏸".cyan(),
            diff.services_to_disable.len()
        );
        for (name, system) in &diff.services_to_disable {
            let scope = if *system { "system" } else { "user" };
            println!("    • {} ({})", name, scope);
        }
    }

    if !diff.services_to_start.is_empty() {
        println!(
            "\n  {} Services to start ({}):",
            "▶".cyan(),
            diff.services_to_start.len()
        );
        for (name, system) in &diff.services_to_start {
            let scope = if *system { "system" } else { "user" };
            println!("    • {} ({})", name, scope);
        }
    }

    if !diff.services_to_stop.is_empty() {
        println!(
            "\n  {} Services to stop ({}):",
            "⏹".cyan(),
            diff.services_to_stop.len()
        );
        for (name, system) in &diff.services_to_stop {
            let scope = if *system { "system" } else { "user" };
            println!("    • {} ({})", name, scope);
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
        options.use_sudo,
        options.user_services,
        options.package_manager_type,
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
        options.use_sudo,
        options.user_services,
        options.package_manager_type,
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
    let symlink_resolution = options.config.get_symlink_resolution()?;
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;

    for file in &diff.files_to_sync {
        // Check if we need to backup
        if file.dest_path.exists() {
            let backup_dir = options.config.get_backup_dir()?;
            let backup_path = backup_dir
                .join(chrono::Local::now().format("%Y%m%d_%H%M%S").to_string())
                .join(
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

    // Add package operations
    for (name, version) in &diff.packages_to_install {
        transaction.add_operation(FileOperation::InstallPackage {
            name: name.clone(),
            version: version.clone(),
        });
    }

    for name in &diff.packages_to_remove {
        transaction.add_operation(FileOperation::RemovePackage { name: name.clone() });
    }

    // Add service operations
    for (name, system) in &diff.services_to_enable {
        transaction.add_operation(FileOperation::EnableService {
            name: name.clone(),
            system: *system,
        });
    }

    for (name, system) in &diff.services_to_disable {
        transaction.add_operation(FileOperation::DisableService {
            name: name.clone(),
            system: *system,
        });
    }

    for (name, system) in &diff.services_to_start {
        transaction.add_operation(FileOperation::StartService {
            name: name.clone(),
            system: *system,
        });
    }

    for (name, system) in &diff.services_to_stop {
        transaction.add_operation(FileOperation::StopService {
            name: name.clone(),
            system: *system,
        });
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
