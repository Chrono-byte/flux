use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::services::{PackageManager, PackageManagerType};
use crate::types::SymlinkResolution;
use crate::utils::error::{DotfilesError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Represents the state of a transaction during its lifecycle
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction has been started
    Started,
    /// All operations validated and prepared
    Prepared,
    /// Changes have been committed to the system
    Committed,
    /// State has been verified after commit
    Verified,
    /// Transaction was rolled back
    RolledBack,
}

/// Represents a single operation within a transaction
#[derive(Debug, Clone)]
pub enum FileOperation {
    /// Create a symlink from source to target
    CreateSymlink {
        source: PathBuf,
        target: PathBuf,
        resolution: SymlinkResolution,
    },
    /// Remove a symlink at target
    RemoveSymlink { target: PathBuf },
    /// Backup existing file and replace with symlink
    BackupAndReplace {
        source: PathBuf,
        target: PathBuf,
        backup_path: PathBuf,
        resolution: SymlinkResolution,
    },
    /// Install a package
    InstallPackage { name: String, version: String },
    /// Remove a package
    RemovePackage { name: String },
}

/// Result of executing an operation
#[derive(Debug, Clone)]
pub struct OperationResult {
    pub operation: FileOperation,
    pub success: bool,
    pub error: Option<String>,
}

/// Main transaction struct that manages atomic operations
pub struct Transaction {
    /// Unique transaction ID
    pub id: String,
    /// Current state of the transaction
    pub state: TransactionState,
    /// Temporary directory for staging changes
    pub temp_dir: PathBuf,
    /// List of operations to perform
    pub operations: Vec<FileOperation>,
    /// Results of executed operations
    pub results: Vec<OperationResult>,
    /// Backup paths created during transaction
    pub backups: Vec<PathBuf>,
    /// Metadata for the transaction
    pub metadata: HashMap<String, String>,
    /// Package manager instance (boxed trait object)
    package_manager: Option<Box<dyn PackageManager>>,
}

impl Transaction {
    /// Begin a new transaction
    pub fn begin(
        temp_dir: PathBuf,
        use_sudo: bool,
        package_manager_type: PackageManagerType,
    ) -> Result<Self> {
        let id = Uuid::new_v4().to_string();

        // Create temp directory if it doesn't exist
        if !temp_dir.exists() {
            fs::create_dir_all(&temp_dir)?;
        }

        // Create package manager based on type
        let package_manager: Option<Box<dyn PackageManager>> =
            Some(package_manager_type.create_manager(use_sudo));

        Ok(Self {
            id,
            state: TransactionState::Started,
            temp_dir,
            operations: Vec::new(),
            results: Vec::new(),
            backups: Vec::new(),
            metadata: HashMap::new(),
            package_manager,
        })
    }

    /// Add an operation to the transaction
    pub fn add_operation(&mut self, operation: FileOperation) {
        self.operations.push(operation);
    }

    /// Validate all operations in the transaction
    pub fn validate(&mut self, _config: &Config) -> Result<()> {
        if self.state != TransactionState::Started {
            return Err(DotfilesError::Config(
                "Transaction must be in Started state to validate".to_string(),
            ));
        }

        // Validate each operation
        for op in &self.operations {
            match op {
                FileOperation::CreateSymlink { source, target, .. } => {
                    // Check source exists
                    if !source.exists() {
                        return Err(DotfilesError::Path(format!(
                            "Source file does not exist: {}",
                            source.display()
                        )));
                    }
                    // Check target parent directory exists or can be created
                    if let Some(parent) = target.parent()
                        && !parent.exists()
                    {
                        // Will be created during prepare
                    }
                }
                FileOperation::RemoveSymlink { target } => {
                    // Check target exists or is a symlink
                    if !target.exists() && !target.is_symlink() {
                        // Target doesn't exist, which is fine for removal
                        // (idempotent operation)
                    }
                }
                FileOperation::BackupAndReplace {
                    source,
                    backup_path,
                    ..
                } => {
                    if !source.exists() {
                        return Err(DotfilesError::Path(format!(
                            "Source file does not exist: {}",
                            source.display()
                        )));
                    }
                    // Check backup directory can be created
                    if let Some(parent) = backup_path.parent()
                        && !parent.exists()
                    {
                        // Will be created during prepare
                    }
                }
                FileOperation::InstallPackage { name, .. } => {
                    // Package validation happens during execution
                    if name.is_empty() {
                        return Err(DotfilesError::Config(
                            "Package name cannot be empty".to_string(),
                        ));
                    }
                }
                FileOperation::RemovePackage { name } => {
                    if name.is_empty() {
                        return Err(DotfilesError::Config(
                            "Package name cannot be empty".to_string(),
                        ));
                    }
                }
            }
        }

        self.state = TransactionState::Prepared;
        Ok(())
    }

    /// Prepare all operations (stage changes in temp directory)
    pub fn prepare(&mut self, _config: &Config) -> Result<()> {
        if self.state != TransactionState::Prepared {
            return Err(DotfilesError::Config(
                "Transaction must be in Prepared state to prepare".to_string(),
            ));
        }

        // For file operations, we'll stage them during commit
        // This phase is mainly for validation and setup
        Ok(())
    }

    /// Commit all operations atomically
    pub fn commit(&mut self, config: &Config, fs_manager: &mut FileSystemManager) -> Result<()> {
        if self.state != TransactionState::Prepared {
            return Err(DotfilesError::Config(
                "Transaction must be in Prepared state to commit".to_string(),
            ));
        }

        // Execute all operations
        for op in self.operations.clone() {
            let result = match &op {
                FileOperation::CreateSymlink {
                    source,
                    target,
                    resolution,
                } => self.execute_create_symlink(source, target, *resolution, config, fs_manager),
                FileOperation::RemoveSymlink { target } => {
                    self.execute_remove_symlink(target, fs_manager)
                }
                FileOperation::BackupAndReplace {
                    source,
                    target,
                    backup_path,
                    resolution,
                } => self.execute_backup_and_replace(
                    source,
                    target,
                    backup_path,
                    *resolution,
                    config,
                    fs_manager,
                ),
                FileOperation::InstallPackage { name, version } => {
                    self.execute_install_package(name, version)?
                }
                FileOperation::RemovePackage { name } => self.execute_remove_package(name)?,
            };

            self.results.push(result.clone());

            // If any operation fails, rollback
            if !result.success {
                self.rollback(config, fs_manager)?;
                return Err(DotfilesError::Config(format!(
                    "Transaction failed: {}",
                    result.error.as_deref().unwrap_or("Unknown error")
                )));
            }
        }

        self.state = TransactionState::Committed;
        Ok(())
    }

    /// Verify that all changes were applied correctly
    pub fn verify(&mut self) -> Result<()> {
        if self.state != TransactionState::Committed {
            return Err(DotfilesError::Config(
                "Transaction must be in Committed state to verify".to_string(),
            ));
        }

        // Verify each operation succeeded
        for result in &self.results {
            if !result.success {
                return Err(DotfilesError::Config(format!(
                    "Verification failed: operation did not succeed: {}",
                    result.error.as_deref().unwrap_or("Unknown error")
                )));
            }

            // Additional verification for file and package operations
            match &result.operation {
                FileOperation::CreateSymlink { target, .. } => {
                    if !target.exists() && !target.is_symlink() {
                        return Err(DotfilesError::Path(format!(
                            "Verification failed: symlink does not exist: {}",
                            target.display()
                        )));
                    }
                }
                FileOperation::RemoveSymlink { target } => {
                    if target.exists() || target.is_symlink() {
                        return Err(DotfilesError::Path(format!(
                            "Verification failed: symlink still exists: {}",
                            target.display()
                        )));
                    }
                }
                FileOperation::InstallPackage { name, .. } => {
                    if let Some(ref pm) = self.package_manager {
                        match pm.is_installed(name) {
                            Ok(true) => {
                                // Package is installed, verification passed
                            }
                            Ok(false) => {
                                return Err(DotfilesError::Config(format!(
                                    "Verification failed: package {} is not installed",
                                    name
                                )));
                            }
                            Err(e) => {
                                // Package manager error, but don't fail verification
                                // (might be unavailable in some environments)
                                log::warn!("Could not verify package installation: {}", e);
                            }
                        }
                    }
                }
                FileOperation::RemovePackage { name } => {
                    if let Some(ref pm) = self.package_manager {
                        match pm.is_installed(name) {
                            Ok(false) => {
                                // Package is not installed, verification passed
                            }
                            Ok(true) => {
                                return Err(DotfilesError::Config(format!(
                                    "Verification failed: package {} is still installed",
                                    name
                                )));
                            }
                            Err(e) => {
                                // Package manager error, but don't fail verification
                                log::warn!("Could not verify package removal: {}", e);
                            }
                        }
                    }
                }
                _ => {
                    // Other operations verified by their success flag
                }
            }
        }

        self.state = TransactionState::Verified;
        Ok(())
    }

    /// Rollback all changes made by this transaction
    pub fn rollback(&mut self, _config: &Config, fs_manager: &mut FileSystemManager) -> Result<()> {
        if self.state == TransactionState::RolledBack {
            return Ok(()); // Already rolled back
        }

        // Rollback in reverse order
        for result in self.results.iter().rev() {
            if result.success {
                match &result.operation {
                    FileOperation::CreateSymlink { target, .. } => {
                        // Remove the symlink we created
                        if target.exists() || target.is_symlink() {
                            let _ = fs_manager.remove_file(target);
                        }
                    }
                    FileOperation::RemoveSymlink { target: _ } => {
                        // Can't easily restore removed symlinks, but we have backups
                        // This would require storing the original state
                    }
                    FileOperation::BackupAndReplace {
                        target,
                        backup_path,
                        ..
                    } => {
                        // Restore from backup
                        if backup_path.exists() {
                            if let Some(parent) = target.parent() {
                                let _ = fs_manager.create_dir_all(parent);
                            }
                            if backup_path.is_dir() {
                                let _ = fs_manager.copy_dir_all(backup_path, target);
                            } else {
                                let _ = fs_manager.copy(backup_path, target);
                            }
                        }
                    }
                    FileOperation::InstallPackage { name, .. } => {
                        // Rollback: remove the package we installed
                        if let Some(ref pm) = self.package_manager {
                            // Check if still installed before attempting removal
                            if let Ok(true) = pm.is_installed(name) {
                                let _ = pm.remove(&[name]);
                            }
                        }
                    }
                    FileOperation::RemovePackage { name, .. } => {
                        // Rollback: reinstall the package we removed
                        // Note: We don't know the exact version that was installed,
                        // so we'll install the latest version
                        if let Some(ref pm) = self.package_manager {
                            // Check if not installed before attempting installation
                            if let Ok(false) = pm.is_installed(name) {
                                let _ = pm.install(&[(name, "latest")]);
                            }
                        }
                    }
                }
            }
        }

        self.state = TransactionState::RolledBack;
        Ok(())
    }

    /// Get list of changes made by this transaction
    #[allow(dead_code)]
    pub fn get_changes(&self) -> Vec<&FileOperation> {
        self.operations.iter().collect()
    }

    /// Clean up temporary files and directories
    pub fn cleanup(&self) -> Result<()> {
        if self.temp_dir.exists() {
            fs::remove_dir_all(&self.temp_dir)?;
        }
        Ok(())
    }

    // Private helper methods for executing operations

    fn execute_create_symlink(
        &mut self,
        source: &Path,
        target: &Path,
        resolution: SymlinkResolution,
        _config: &Config,
        fs_manager: &mut FileSystemManager,
    ) -> OperationResult {
        // Create parent directory if needed
        if let Some(parent) = target.parent()
            && let Err(e) = fs_manager.create_dir_all(parent)
        {
            return OperationResult {
                operation: FileOperation::CreateSymlink {
                    source: source.to_path_buf(),
                    target: target.to_path_buf(),
                    resolution,
                },
                success: false,
                error: Some(format!("Failed to create parent directory: {}", e)),
            };
        }

        // Create symlink using atomic approach
        let link_target = match resolution {
            SymlinkResolution::Auto | SymlinkResolution::Relative => {
                pathdiff::diff_paths(source, target.parent().unwrap())
                    .unwrap_or_else(|| source.to_path_buf())
            }
            SymlinkResolution::Absolute => source.to_path_buf(),
            SymlinkResolution::Follow => pathdiff::diff_paths(source, target.parent().unwrap())
                .unwrap_or_else(|| source.to_path_buf()),
            SymlinkResolution::Replace => {
                // For Replace, we copy instead of symlink
                let temp_path = target.with_extension("flux-temp-copy");
                if let Err(e) = fs_manager.copy(source, &temp_path) {
                    return OperationResult {
                        operation: FileOperation::CreateSymlink {
                            source: source.to_path_buf(),
                            target: target.to_path_buf(),
                            resolution,
                        },
                        success: false,
                        error: Some(format!("Failed to copy file: {}", e)),
                    };
                }
                if let Err(e) = fs_manager.rename(&temp_path, target) {
                    return OperationResult {
                        operation: FileOperation::CreateSymlink {
                            source: source.to_path_buf(),
                            target: target.to_path_buf(),
                            resolution,
                        },
                        success: false,
                        error: Some(format!("Failed to rename temp file: {}", e)),
                    };
                }
                return OperationResult {
                    operation: FileOperation::CreateSymlink {
                        source: source.to_path_buf(),
                        target: target.to_path_buf(),
                        resolution,
                    },
                    success: true,
                    error: None,
                };
            }
        };

        // Create temp symlink first
        let temp_link_path = target.with_extension(format!(
            "{}.flux-temp",
            target.extension().map_or("", |s| s.to_str().unwrap_or(""))
        ));

        // Remove old temp link if exists
        let _ = fs_manager.remove_file(&temp_link_path);

        if let Err(e) = fs_manager.symlink(&link_target, &temp_link_path) {
            return OperationResult {
                operation: FileOperation::CreateSymlink {
                    source: source.to_path_buf(),
                    target: target.to_path_buf(),
                    resolution,
                },
                success: false,
                error: Some(format!("Failed to create temp symlink: {}", e)),
            };
        }

        // Atomically rename
        if let Err(e) = fs_manager.rename(&temp_link_path, target) {
            return OperationResult {
                operation: FileOperation::CreateSymlink {
                    source: source.to_path_buf(),
                    target: target.to_path_buf(),
                    resolution,
                },
                success: false,
                error: Some(format!("Failed to rename temp symlink: {}", e)),
            };
        }

        OperationResult {
            operation: FileOperation::CreateSymlink {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                resolution,
            },
            success: true,
            error: None,
        }
    }

    fn execute_remove_symlink(
        &mut self,
        target: &Path,
        fs_manager: &mut FileSystemManager,
    ) -> OperationResult {
        let target_path = target.to_path_buf();
        if let Err(e) = fs_manager.remove_file(&target_path) {
            OperationResult {
                operation: FileOperation::RemoveSymlink {
                    target: target_path,
                },
                success: false,
                error: Some(format!("Failed to remove symlink: {}", e)),
            }
        } else {
            OperationResult {
                operation: FileOperation::RemoveSymlink {
                    target: target_path,
                },
                success: true,
                error: None,
            }
        }
    }

    fn execute_backup_and_replace(
        &mut self,
        source: &Path,
        target: &Path,
        backup_path: &Path,
        resolution: SymlinkResolution,
        config: &Config,
        fs_manager: &mut FileSystemManager,
    ) -> OperationResult {
        // Create backup
        if target.exists() {
            if let Some(parent) = backup_path.parent()
                && let Err(e) = fs_manager.create_dir_all(parent)
            {
                return OperationResult {
                    operation: FileOperation::BackupAndReplace {
                        source: source.to_path_buf(),
                        target: target.to_path_buf(),
                        backup_path: backup_path.to_path_buf(),
                        resolution,
                    },
                    success: false,
                    error: Some(format!("Failed to create backup directory: {}", e)),
                };
            }

            if target.is_dir() {
                if let Err(e) = fs_manager.copy_dir_all(target, backup_path) {
                    return OperationResult {
                        operation: FileOperation::BackupAndReplace {
                            source: source.to_path_buf(),
                            target: target.to_path_buf(),
                            backup_path: backup_path.to_path_buf(),
                            resolution,
                        },
                        success: false,
                        error: Some(format!("Failed to backup directory: {}", e)),
                    };
                }
            } else if let Err(e) = fs_manager.copy(target, backup_path) {
                return OperationResult {
                    operation: FileOperation::BackupAndReplace {
                        source: source.to_path_buf(),
                        target: target.to_path_buf(),
                        backup_path: backup_path.to_path_buf(),
                        resolution,
                    },
                    success: false,
                    error: Some(format!("Failed to backup file: {}", e)),
                };
            }

            self.backups.push(backup_path.to_path_buf());
        }

        // Now create symlink (or copy for Replace)
        let result = self.execute_create_symlink(source, target, resolution, config, fs_manager);
        if !result.success {
            return result;
        }

        OperationResult {
            operation: FileOperation::BackupAndReplace {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                backup_path: backup_path.to_path_buf(),
                resolution,
            },
            success: true,
            error: None,
        }
    }

    fn execute_install_package(&mut self, name: &str, version: &str) -> Result<OperationResult> {
        let package_name = name.to_string();
        let package_version = version.to_string();

        if let Some(ref pm) = self.package_manager {
            // Check if already installed
            match pm.is_installed(&package_name) {
                Ok(true) => {
                    // Already installed, check version if specified
                    if package_version != "latest" {
                        if let Ok(Some(installed_version)) = pm.get_version(&package_name)
                            && installed_version == package_version
                        {
                            // Already at correct version
                            return Ok(OperationResult {
                                operation: FileOperation::InstallPackage {
                                    name: package_name,
                                    version: package_version,
                                },
                                success: true,
                                error: None,
                            });
                        }
                    } else {
                        // Already installed, no version specified
                        return Ok(OperationResult {
                            operation: FileOperation::InstallPackage {
                                name: package_name,
                                version: package_version,
                            },
                            success: true,
                            error: None,
                        });
                    }
                }
                Ok(false) => {
                    // Not installed, proceed with installation
                }
                Err(e) => {
                    // Package manager error
                    return Ok(OperationResult {
                        operation: FileOperation::InstallPackage {
                            name: package_name.clone(),
                            version: package_version.clone(),
                        },
                        success: false,
                        error: Some(format!("Failed to check if package is installed: {}", e)),
                    });
                }
            }

            // Install the package
            match pm.install(&[(&package_name, &package_version)]) {
                Ok(()) => Ok(OperationResult {
                    operation: FileOperation::InstallPackage {
                        name: package_name,
                        version: package_version,
                    },
                    success: true,
                    error: None,
                }),
                Err(e) => Ok(OperationResult {
                    operation: FileOperation::InstallPackage {
                        name: package_name,
                        version: package_version,
                    },
                    success: false,
                    error: Some(format!("Failed to install package: {}", e)),
                }),
            }
        } else {
            // No package manager available
            Ok(OperationResult {
                operation: FileOperation::InstallPackage {
                    name: package_name,
                    version: package_version,
                },
                success: false,
                error: Some("Package manager not available".to_string()),
            })
        }
    }

    fn execute_remove_package(&mut self, name: &str) -> Result<OperationResult> {
        let package_name = name.to_string();

        if let Some(ref pm) = self.package_manager {
            // Check if installed
            match pm.is_installed(&package_name) {
                Ok(false) => {
                    // Not installed, nothing to do
                    return Ok(OperationResult {
                        operation: FileOperation::RemovePackage { name: package_name },
                        success: true,
                        error: None,
                    });
                }
                Ok(true) => {
                    // Installed, proceed with removal
                }
                Err(e) => {
                    // Package manager error
                    return Ok(OperationResult {
                        operation: FileOperation::RemovePackage {
                            name: package_name.clone(),
                        },
                        success: false,
                        error: Some(format!("Failed to check if package is installed: {}", e)),
                    });
                }
            }

            // Remove the package
            match pm.remove(&[&package_name]) {
                Ok(()) => Ok(OperationResult {
                    operation: FileOperation::RemovePackage { name: package_name },
                    success: true,
                    error: None,
                }),
                Err(e) => Ok(OperationResult {
                    operation: FileOperation::RemovePackage { name: package_name },
                    success: false,
                    error: Some(format!("Failed to remove package: {}", e)),
                }),
            }
        } else {
            // No package manager available
            Ok(OperationResult {
                operation: FileOperation::RemovePackage { name: package_name },
                success: false,
                error: Some("Package manager not available".to_string()),
            })
        }
    }
}
