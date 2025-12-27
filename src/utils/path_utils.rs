use crate::utils::error::Result;
use std::path::{Path, PathBuf};

/// Normalize a path by canonicalizing it, falling back to the path itself if canonicalization fails.
#[must_use]
pub fn normalize_path(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(_) => path.to_path_buf(),
    }
}

/// Check if two files have different content.
///
/// # Errors
///
/// Returns an error if there are issues reading the files.
pub fn files_differ(path1: &Path, path2: &Path) -> Result<bool> {
    use std::fs;

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

/// Resolve a symlink target to an absolute path.
///
/// If the target is already absolute, returns it as-is.
/// If the target is relative, resolves it relative to the symlink's parent directory.
#[must_use]
pub fn resolve_symlink_target(symlink_path: &Path, link_target: &Path) -> PathBuf {
    if link_target.is_absolute() {
        link_target.to_path_buf()
    } else {
        match symlink_path.parent() {
            Some(parent) => parent.join(link_target),
            None => link_target.to_path_buf(),
        }
    }
}

/// Check if a symlink points to the correct target by comparing normalized paths.
#[must_use]
pub fn symlink_points_to_correct_target(
    symlink_path: &Path,
    link_target: &Path,
    expected_target: &Path,
) -> bool {
    let resolved_target = resolve_symlink_target(symlink_path, link_target);
    let normalized_target = normalize_path(&resolved_target);
    let normalized_expected = normalize_path(expected_target);
    normalized_target == normalized_expected
}

/// Check if any parent directory of the given path is a symlink.
/// Returns the symlinked parent directory path if found, and the resolved target.
/// This helps detect cases where a directory is symlinked instead of individual files.
#[must_use]
pub fn find_symlinked_parent(file_path: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut current = file_path.parent()?;

    loop {
        // Check if current directory is a symlink
        if let Ok(link_target) = std::fs::read_link(current) {
            let resolved_target = resolve_symlink_target(current, &link_target);
            let normalized_target = normalize_path(&resolved_target);
            return Some((current.to_path_buf(), normalized_target));
        }

        // Move up to parent directory
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    None
}
