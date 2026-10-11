use crate::config::Config;
use crate::file_manager::FileSystemManager;
use crate::types::SymlinkResolution;
use crate::utils::error::{DotfilesError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Represents the state of a transaction during its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Source path for the symlink.
    source: PathBuf,
    /// Target path where the symlink will be created.
    target: PathBuf,
    /// Symlink resolution strategy to use.
    resolution: SymlinkResolution,
  },
  /// Remove a symlink at target
  RemoveSymlink {
    /// Target path of the symlink to remove.
    target: PathBuf,
  },
}

/// Result of executing an operation.
#[derive(Debug, Clone)]
pub struct OperationResult {
  /// The operation that was executed
  pub operation: FileOperation,
  /// Whether the operation succeeded
  pub success: bool,
  /// Error message if the operation failed
  pub error: Option<String>,
}

/// Manages atomic file operations within a transaction.
///
/// See [`TransactionState`] for the transaction lifecycle.
#[derive(Debug)]
pub struct Transaction {
  /// Unique transaction identifier
  pub id: String,
  /// Current state in the transaction lifecycle
  pub state: TransactionState,
  /// Temporary directory for staging changes
  pub temp_dir: PathBuf,
  /// Operations to be executed
  pub operations: Vec<FileOperation>,
  /// Results from executed operations
  pub results: Vec<OperationResult>,
  /// Additional metadata (description, profile, timestamp, etc.)
  pub metadata: HashMap<String, String>,
}

impl Default for Transaction {
  fn default() -> Self {
    Self {
      id: Uuid::new_v4().to_string(),
      state: TransactionState::Started,
      temp_dir: PathBuf::new(),
      operations: Vec::new(),
      results: Vec::new(),
      metadata: HashMap::new(),
    }
  }
}

impl Transaction {
  /// Begin a new transaction
  ///
  /// # Errors
  ///
  /// Returns an error if the temporary directory cannot be created.
  pub fn begin(temp_dir: PathBuf) -> Result<Self> {
    if !temp_dir.exists() {
      fs::create_dir_all(&temp_dir)?;
    }

    Ok(Self {
      temp_dir,
      ..Default::default()
    })
  }

  /// Add an operation to the transaction
  pub fn add_operation(
    &mut self,
    operation: FileOperation,
  ) {
    self.operations.push(operation);
  }

  /// Validate the transaction. Runs before preparation.
  ///
  /// # Errors
  ///
  /// Returns an error if the transaction is not in the correct state or if validation fails.
  pub fn validate(
    &mut self,
    _config: &Config,
  ) -> Result<()> {
    if self.state != TransactionState::Started {
      return Err(DotfilesError::Config(
        "Transaction must be in Started state to validate".to_string(),
      ));
    }

    for op in &self.operations {
      match op {
        FileOperation::CreateSymlink { source, .. } => {
          if !source.exists() {
            return Err(DotfilesError::Path(format!(
              "Source file does not exist: {}",
              source.display()
            )));
          }
        }
        FileOperation::RemoveSymlink { .. } => {}
      }
    }

    self.state = TransactionState::Prepared;
    Ok(())
  }

  /// Prepare all operations (stage changes in temp directory)
  ///
  /// # Errors
  ///
  /// Returns an error if the transaction is not in the correct state or if preparation fails.
  pub fn prepare(&self) -> Result<()> {
    if self.state != TransactionState::Prepared {
      return Err(DotfilesError::Config(
        "Transaction must be in Prepared state to prepare".to_string(),
      ));
    }

    Ok(())
  }

  /// Commit all operations atomically
  ///
  /// # Errors
  ///
  /// Returns an error if the transaction is not in the correct state or if any operation fails.
  pub fn commit(
    &mut self,
    config: &Config,
    fs_manager: &mut FileSystemManager,
  ) -> Result<()> {
    if self.state != TransactionState::Prepared {
      return Err(DotfilesError::Config(
        "Transaction must be in Prepared state to commit".to_string(),
      ));
    }

    for (operation_index, op) in self.operations.clone().into_iter().enumerate() {
      let result = match &op {
        FileOperation::CreateSymlink {
          source,
          target,
          resolution,
        } => Self::execute_create_symlink(
          source,
          target,
          *resolution,
          &self
            .temp_dir
            .join(format!("operation-{operation_index}.backup")),
          config,
          fs_manager,
        ),
        FileOperation::RemoveSymlink { target } => Self::execute_remove_symlink(
          target,
          &self
            .temp_dir
            .join(format!("operation-{operation_index}.backup")),
          fs_manager,
        ),
      };

      self.results.push(result.clone());

      if !result.success {
        self.rollback(config, fs_manager);
        let error_message = result.error.as_deref().unwrap_or("Unknown error");
        return Err(DotfilesError::Config(format!(
          "Transaction failed: {error_message}"
        )));
      }
    }

    self.state = TransactionState::Committed;
    Ok(())
  }

  /// Verify that all changes were applied correctly
  /// Verify that all operations completed successfully.
  ///
  /// # Errors
  ///
  /// Returns an error if the transaction is not in the correct state or if verification fails.
  pub fn verify(&mut self) -> Result<()> {
    if self.state != TransactionState::Committed {
      return Err(DotfilesError::Config(
        "Transaction must be in Committed state to verify".to_string(),
      ));
    }

    let mut targets_created_after_removal = std::collections::HashSet::new();
    for (i, result) in self.results.iter().enumerate() {
      if let FileOperation::RemoveSymlink { target } = &result.operation {
        if let Some(next_result) = self.results.get(i + 1)
          && let FileOperation::CreateSymlink {
            target: create_target,
            ..
          } = &next_result.operation
          && target == create_target
        {
          let _ = targets_created_after_removal.insert(target.clone());
        }
      }
    }

    for result in &self.results {
      if !result.success {
        let error_message = result.error.as_deref().unwrap_or("Unknown error");
        return Err(DotfilesError::Config(format!(
          "Verification failed: operation did not succeed: {error_message}"
        )));
      }

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
          if !targets_created_after_removal.contains(target)
            && (target.exists() || target.is_symlink())
          {
            return Err(DotfilesError::Path(format!(
              "Verification failed: symlink still exists: {}",
              target.display()
            )));
          }
        }
      }
    }

    self.state = TransactionState::Verified;
    Ok(())
  }

  /// Rollback all changes made by this transaction
  pub fn rollback(
    &mut self,
    _config: &Config,
    fs_manager: &mut FileSystemManager,
  ) {
    if self.state == TransactionState::RolledBack {
      return;
    }

    for (operation_index, result) in self.results.iter().enumerate().rev() {
      if result.success {
        match &result.operation {
          FileOperation::CreateSymlink { target, .. } | FileOperation::RemoveSymlink { target } => {
            if target.exists() || target.is_symlink() {
              let _ = fs_manager.remove_file(target);
            }
            let backup_path = self
              .temp_dir
              .join(format!("operation-{operation_index}.backup"));
            if backup_path.exists() || backup_path.is_symlink() {
              let _ = fs_manager.rename(&backup_path, target);
            }
          }
        }
      }
    }

    self.state = TransactionState::RolledBack;
  }

  /// Clean up temporary files and directories
  ///
  /// # Errors
  ///
  /// Returns an error if cleanup fails.
  pub fn cleanup(&self) -> Result<()> {
    if self.temp_dir.exists() {
      fs::remove_dir_all(&self.temp_dir)?;
    }
    Ok(())
  }

  fn execute_create_symlink(
    source: &Path,
    target: &Path,
    resolution: SymlinkResolution,
    backup_path: &Path,
    _config: &Config,
    fs_manager: &mut FileSystemManager,
  ) -> OperationResult {
    if let Some(parent) = target.parent()
      && let Err(error) = fs_manager.create_dir_all(parent)
    {
      return OperationResult {
        operation: FileOperation::CreateSymlink {
          source: source.to_path_buf(),
          target: target.to_path_buf(),
          resolution,
        },
        success: false,
        error: Some(format!("Failed to create parent directory: {error}")),
      };
    }

    // Create the replacement at a sibling path first so the final rename is
    // atomic on the destination filesystem.
    let ext_str = target
      .extension()
      .and_then(|ext| ext.to_str())
      .unwrap_or_default();
    let temp_path = target.with_extension(format!("{ext_str}.flux-temp"));
    let _ = fs_manager.remove_file(&temp_path);

    if let Err(error) = Self::stage_replacement(source, target, resolution, &temp_path, fs_manager)
    {
      return OperationResult {
        operation: FileOperation::CreateSymlink {
          source: source.to_path_buf(),
          target: target.to_path_buf(),
          resolution,
        },
        success: false,
        error: Some(format!(
          "Failed to stage replacement for {}: {error}",
          target.display()
        )),
      };
    }

    let had_original = target.exists() || target.is_symlink();
    if had_original && let Err(error) = fs_manager.rename(target, backup_path) {
      let _ = fs_manager.remove_file(&temp_path);
      return OperationResult {
        operation: FileOperation::CreateSymlink {
          source: source.to_path_buf(),
          target: target.to_path_buf(),
          resolution,
        },
        success: false,
        error: Some(format!(
          "Failed to back up existing destination {}: {error}",
          target.display()
        )),
      };
    }

    if let Err(error) = fs_manager.rename(&temp_path, target) {
      if had_original {
        let _ = fs_manager.rename(backup_path, target);
      }
      return OperationResult {
        operation: FileOperation::CreateSymlink {
          source: source.to_path_buf(),
          target: target.to_path_buf(),
          resolution,
        },
        success: false,
        error: Some(format!(
          "Failed to install staged replacement for {}: {error}",
          target.display()
        )),
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

  fn stage_replacement(
    source: &Path,
    target: &Path,
    resolution: SymlinkResolution,
    temp_path: &Path,
    fs_manager: &mut FileSystemManager,
  ) -> Result<()> {
    if resolution == SymlinkResolution::Replace {
      return if source.is_dir() {
        fs_manager.copy_dir_all(source, temp_path)
      } else {
        fs_manager.copy(source, temp_path)
      };
    }

    let link_target = match resolution {
      SymlinkResolution::Auto | SymlinkResolution::Relative | SymlinkResolution::Follow => {
        target.parent().map_or_else(
          || source.to_path_buf(),
          |parent| pathdiff::diff_paths(source, parent).unwrap_or_else(|| source.to_path_buf()),
        )
      }
      SymlinkResolution::Absolute => source.to_path_buf(),
      SymlinkResolution::Replace => unreachable!(),
    };
    fs_manager.symlink(&link_target, temp_path)
  }

  fn execute_remove_symlink(
    target: &Path,
    backup_path: &Path,
    fs_manager: &mut FileSystemManager,
  ) -> OperationResult {
    let target_path = target.to_path_buf();
    if (target.exists() || target.is_symlink())
      && let Err(error) = fs_manager.rename(&target_path, backup_path)
    {
      OperationResult {
        operation: FileOperation::RemoveSymlink {
          target: target_path,
        },
        success: false,
        error: Some(format!("Failed to remove symlink: {error}")),
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
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::utils::dry_run::DryRun;

  #[test]
  fn rollback_restores_destination_replaced_before_later_failure() -> Result<()> {
    let root = tempfile::tempdir()?;
    let transaction_dir = root.path().join("transaction");
    let source = root.path().join("source");
    let target = root.path().join("target");
    let blocked_parent = root.path().join("blocked-parent");
    let failing_target = blocked_parent.join("target");
    fs::write(&source, "replacement")?;
    fs::write(&target, "original")?;
    fs::write(&blocked_parent, "not a directory")?;

    let mut transaction = Transaction::begin(transaction_dir)?;
    transaction.add_operation(FileOperation::CreateSymlink {
      source: source.clone(),
      target: target.clone(),
      resolution: SymlinkResolution::Auto,
    });
    transaction.add_operation(FileOperation::CreateSymlink {
      source,
      target: failing_target,
      resolution: SymlinkResolution::Auto,
    });

    let config = Config::default();
    transaction.validate(&config)?;
    transaction.prepare()?;
    let mut dry_run = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run, false);

    assert!(
      transaction
        .commit(&config, &mut fs_manager)
        .is_err()
    );
    assert_eq!(transaction.state, TransactionState::RolledBack);
    assert!(!target.is_symlink());
    assert_eq!(fs::read_to_string(target)?, "original");
    Ok(())
  }

  #[test]
  fn replace_resolution_copies_directories() -> Result<()> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir_all(source.join("nested"))?;
    fs::write(source.join("nested/file"), "replacement")?;

    let mut transaction = Transaction::begin(root.path().join("transaction"))?;
    transaction.add_operation(FileOperation::CreateSymlink {
      source,
      target: target.clone(),
      resolution: SymlinkResolution::Replace,
    });

    let config = Config::default();
    transaction.validate(&config)?;
    transaction.prepare()?;
    let mut dry_run = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run, false);
    transaction.commit(&config, &mut fs_manager)?;

    assert_eq!(
      fs::read_to_string(target.join("nested/file"))?,
      "replacement"
    );
    Ok(())
  }
  #[cfg(unix)]
  #[test]
  fn rollback_restores_removed_symlink_after_later_failure() -> Result<()> {
    let root = tempfile::tempdir()?;
    let target = root.path().join("link");
    let original = root.path().join("missing-original");
    std::os::unix::fs::symlink(&original, &target)?;
    let source = root.path().join("source");
    let blocker = root.path().join("blocker");
    fs::write(&source, "replacement")?;
    fs::write(&blocker, "not a directory")?;
    let mut transaction = Transaction::begin(root.path().join("transaction"))?;
    transaction.add_operation(FileOperation::RemoveSymlink {
      target: target.clone(),
    });
    transaction.add_operation(FileOperation::CreateSymlink {
      source,
      target: blocker.join("link"),
      resolution: SymlinkResolution::Auto,
    });
    let config = Config::default();
    transaction.validate(&config)?;
    transaction.prepare()?;
    let mut dry_run = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run, false);
    assert!(
      transaction
        .commit(&config, &mut fs_manager)
        .is_err()
    );
    assert_eq!(transaction.state, TransactionState::RolledBack);
    assert_eq!(fs::read_link(&target)?, original);
    transaction.rollback(&config, &mut fs_manager);
    assert!(target.is_symlink());
    Ok(())
  }

  #[cfg(unix)]
  #[test]
  fn committed_symlink_removal_preserves_the_referent() -> Result<()> {
    let root = tempfile::tempdir()?;
    let source = root.path().join("source");
    let target = root.path().join("link");
    fs::write(&source, "original")?;
    std::os::unix::fs::symlink(&source, &target)?;
    let mut transaction = Transaction::begin(root.path().join("transaction"))?;
    transaction.add_operation(FileOperation::RemoveSymlink {
      target: target.clone(),
    });
    let config = Config::default();
    transaction.validate(&config)?;
    transaction.prepare()?;
    let mut dry_run = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run, false);
    transaction.commit(&config, &mut fs_manager)?;
    transaction.verify()?;
    transaction.cleanup()?;
    assert!(!target.is_symlink());
    assert_eq!(fs::read_to_string(source)?, "original");
    Ok(())
  }
  #[test]
  fn removing_an_absent_symlink_is_idempotent() -> Result<()> {
    let root = tempfile::tempdir()?;
    let target = root.path().join("absent");
    let mut transaction = Transaction::begin(root.path().join("transaction"))?;
    transaction.add_operation(FileOperation::RemoveSymlink {
      target: target.clone(),
    });
    let config = Config::default();
    transaction.validate(&config)?;
    transaction.prepare()?;
    let mut dry_run = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run, false);
    transaction.commit(&config, &mut fs_manager)?;
    transaction.verify()?;
    transaction.rollback(&config, &mut fs_manager);
    assert!(!target.exists());
    assert_eq!(transaction.state, TransactionState::RolledBack);
    Ok(())
  }
}
