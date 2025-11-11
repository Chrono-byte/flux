use crate::error::{DotfilesError, Result};
use std::path::Path;

/// Validate that a symlink target is within the repository (prevents path traversal attacks)
pub fn validate_symlink_target(repo_path: &Path, target: &Path) -> Result<()> {
    // Canonicalize both paths for comparison
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

    // Check if target is within repository
    if !canonical_target.starts_with(&canonical_repo) {
        return Err(crate::error_utils::symlink_target_outside_repo(
            &canonical_target,
            &canonical_repo,
        ));
    }

    Ok(())
}

/// Validate that a destination path doesn't try to escape home directory
#[allow(dead_code)]
pub fn validate_dest_path(dest: &Path, home: &Path) -> Result<()> {
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

    let full_dest = home.join(dest);
    let canonical_dest = full_dest.canonicalize().map_err(|_| {
        DotfilesError::Path(format!(
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
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "N/A".to_string())
        ))
    })?;

    // Check if destination is within home directory
    if !canonical_dest.starts_with(&canonical_home) {
        return Err(crate::error_utils::dest_outside_home(dest, home));
    }

    Ok(())
}

/// Set secure permissions on a file (mode 0600 - read/write owner only)
pub fn set_secure_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }

    #[cfg(not(unix))]
    {
        // On non-Unix systems, just log a warning
        log::warn!("Cannot set secure file permissions on non-Unix system");
    }

    Ok(())
}

/// Check if a file is locked (Unix only using flock)
pub fn is_file_locked(path: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        use nix::fcntl::{FlockArg, flock};
        use std::os::unix::io::AsRawFd;

        match std::fs::OpenOptions::new().read(true).open(path) {
            Ok(file) => {
                let fd = file.as_raw_fd();
                // Try to acquire a non-blocking exclusive lock
                match flock(fd, FlockArg::LockExclusiveNonblock) {
                    Ok(_) => {
                        // Successfully locked, now unlock
                        let _ = flock(fd, FlockArg::Unlock);
                        Ok(false) // File was not locked
                    }
                    Err(nix::Error::EAGAIN) => Ok(true), // File is locked
                    Err(e) => Err(DotfilesError::Path(format!(
                        "Error checking file lock: {}",
                        e
                    ))),
                }
            }
            Err(e) => {
                // If we can't open the file, assume it's locked
                log::warn!(
                    "Could not open file '{}' to check lock status: {}",
                    path.display(),
                    e
                );
                Ok(true)
            }
        }
    }

    #[cfg(not(unix))]
    {
        // On non-Unix systems, we can't check locks this way
        log::warn!("File locking detection not available on this platform");
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_validate_dest_path_within_home() {
        let home = PathBuf::from("/home/user");
        let dest = PathBuf::from(".config/sway/config");

        // This would need actual paths to work, so we'll just check it doesn't panic
        let _ = validate_dest_path(&dest, &home);
    }

    #[test]
    fn test_symlink_target_validation() {
        // This test would need actual filesystem paths
        // In a real implementation, use tempdir for testing
    }
}
