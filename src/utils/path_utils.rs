use crate::utils::error::Result;
use std::path::{Path, PathBuf};

/// Normalize a path by canonicalizing it, falling back to the path itself if canonicalization fails.
pub fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Check if two files have different content.
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
pub fn resolve_symlink_target(symlink_path: &Path, link_target: &Path) -> PathBuf {
    if link_target.is_absolute() {
        link_target.to_path_buf()
    } else {
        symlink_path
            .parent()
            .map(|p| p.join(link_target))
            .unwrap_or_else(|| link_target.to_path_buf())
    }
}

/// Check if a symlink points to the correct target by comparing normalized paths.
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

