use crate::utils::error::{DotfilesError, Result};
use crate::utils::error_utils;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// Validate that a symlink target is within the repository (prevents path traversal attacks)
///
/// # Errors
///
/// Returns an error if the repository path or target path cannot be canonicalized,
/// or if the target path is outside the repository directory.
pub fn validate_symlink_target(
  repo_path: &Path,
  target: &Path,
) -> Result<()> {
  let canonical_repo = repo_path.canonicalize().map_err(|_| {
    DotfilesError::Path(format!(
      "What: Cannot validate repository path\n  \
             Path: {}\n  \
             Why: Path cannot be resolved to an absolute path (may not exist or permission denied)\n  \
             💡 Solution:\n    \
             - Verify repository path exists: `ls -la {}`\n    \
             - Check directory permissions: `ls -ld {}`\n    \
             - Ensure repository path is set correctly in config",
      repo_path.display(),
      repo_path.display(),
      repo_path.display()
    ))
  })?;

  let canonical_target = target.canonicalize().map_err(|_| {
    DotfilesError::Path(format!(
      "What: Cannot resolve symlink target path\n  \
             Path: {}\n  \
             Why: Path cannot be made absolute (file may not exist or permission denied)\n  \
             💡 Solution:\n    \
             - Verify the target file exists and is readable\n    \
             - Check permissions: `ls -la {}`\n    \
             - Ensure path is correct in configuration",
      target.display(),
      target.display()
    ))
  })?;

  if !canonical_target.starts_with(&canonical_repo) {
    return Err(error_utils::symlink_target_outside_repo(
      &canonical_target,
      &canonical_repo,
    ));
  }

  Ok(())
}

/// Validate that a destination path doesn't try to escape home directory
///
/// # Errors
///
/// Returns an error if the home directory or destination path cannot be canonicalized,
/// or if the destination path is outside the home directory.
pub fn validate_dest_path(
  dest: &Path,
  home: &Path,
) -> Result<()> {
  let canonical_home = home.canonicalize().map_err(|_| {
    DotfilesError::Path(format!(
      "What: Cannot validate home directory\n  \
             Path: {}\n  \
             Why: Home directory path cannot be resolved (may not exist or permission denied)\n  \
             💡 Solution:\n    \
             - Verify home directory exists: `ls -la {}`\n    \
             - Check $HOME is set: `echo $HOME`\n    \
             - Ensure permissions allow reading: `ls -ld {}`",
      home.display(),
      home.display(),
      home.display()
    ))
  })?;

  let full_dest = if dest.is_absolute() {
    dest.to_path_buf()
  } else {
    home.join(dest)
  };
  let Ok(canonical_dest) = canonicalize_destination_location(&full_dest) else {
    return Err(DotfilesError::Path(format!(
      "What: Cannot resolve destination path\n  \
                 Destination: {}\n  \
                 Why: Path cannot be made absolute or does not exist\n  \
                 💡 Solution:\n    \
                 - Verify the parent directory exists: `ls -la {}`\n    \
                 - Check path does not contain invalid characters\n    \
                 - Ensure path is relative to home directory",
      full_dest.display(),
      full_dest
        .parent()
        .map_or_else(|| "N/A".to_string(), |parent| parent.display().to_string())
    )));
  };

  if !canonical_dest.starts_with(&canonical_home) {
    return Err(error_utils::dest_outside_home(dest, home));
  }

  Ok(())
}

fn canonicalize_destination_location(path: &Path) -> std::io::Result<PathBuf> {
  match (path.parent(), path.file_name()) {
    (Some(parent), Some(name)) => {
      let mut canonical = canonicalize_with_missing_tail(parent)?;
      canonical.push(name);
      Ok(normalize_nonexistent_path(&canonical))
    }
    _ => canonicalize_with_missing_tail(path),
  }
}

/// Validate that a configured repository entry stays within the repository.
///
/// Unlike [`validate_symlink_target`], this also supports entries whose final
/// path does not exist yet.
///
/// # Errors
///
/// Returns an error if either path cannot be resolved or the entry escapes the
/// repository directory.
pub fn validate_repo_entry_path(
  repo: &Path,
  entry: &Path,
) -> Result<()> {
  let canonical_repo = canonicalize_with_missing_tail(repo)?;
  let canonical_entry = canonicalize_with_missing_tail(entry)?;

  if !canonical_entry.starts_with(&canonical_repo) {
    return Err(error_utils::symlink_target_outside_repo(
      &canonical_entry,
      &canonical_repo,
    ));
  }

  Ok(())
}

fn canonicalize_with_missing_tail(path: &Path) -> std::io::Result<PathBuf> {
  let mut candidate = path;
  let mut missing = Vec::new();

  loop {
    match candidate.canonicalize() {
      Ok(mut canonical) => {
        for component in missing.iter().rev() {
          canonical.push(component);
        }
        return Ok(normalize_nonexistent_path(&canonical));
      }
      Err(error) if error.kind() == ErrorKind::NotFound => {
        let Some(name) = candidate.file_name() else {
          return Err(error);
        };
        missing.push(name.to_os_string());
        let Some(parent) = candidate.parent() else {
          return Err(error);
        };
        candidate = parent;
      }
      Err(error) => return Err(error),
    }
  }
}

fn normalize_nonexistent_path(path: &Path) -> PathBuf {
  let mut normalized = PathBuf::new();

  for component in path.components() {
    match component {
      Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
      Component::RootDir => normalized.push(Path::new("/")),
      Component::CurDir => {}
      Component::ParentDir => {
        let _ = normalized.pop();
      }
      Component::Normal(segment) => normalized.push(segment),
    }
  }

  normalized
}

/// Set secure permissions on a file (mode 0600 - read/write owner only)
///
/// # Errors
///
/// Returns an error if setting file permissions fails (Unix systems only).
pub fn set_secure_permissions(path: &Path) -> Result<()> {
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)?;
  }

  #[cfg(not(unix))]
  {
    log::warn!("Cannot set secure file permissions on non-Unix system");
  }

  Ok(())
}

/// Check if a file is locked (Unix only using flock)
///
/// # Errors
///
/// Returns an error if the file cannot be opened or if there's an error checking the lock.
pub fn is_file_locked(path: &Path) -> Result<bool> {
  #[cfg(unix)]
  {
    use nix::fcntl::{Flock, FlockArg};

    match std::fs::OpenOptions::new().read(true).open(path) {
      Ok(file) => match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
        Ok(_lock) => Ok(false),
        Err((_, nix::Error::EAGAIN)) => Ok(true),
        Err((_, error)) => Err(DotfilesError::Path(format!(
          "Error checking file lock: {error}"
        ))),
      },
      Err(error) => {
        log::warn!(
          "Could not open file '{}' to check lock status: {}",
          path.display(),
          error
        );
        Ok(true)
      }
    }
  }

  #[cfg(not(unix))]
  {
    log::warn!("File locking detection not available on this platform");
    Ok(false)
  }
}
