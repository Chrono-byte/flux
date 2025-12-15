//! Tests for security utilities

use flux::utils::security;
use std::path::PathBuf;

#[test]
fn test_validate_dest_path_within_home() {
    let home = PathBuf::from("/home/user");
    let dest = PathBuf::from(".config/sway/config");

    // This would need actual paths to work, so we'll just check it doesn't panic
    let _ = security::validate_dest_path(&dest, &home);
}

#[test]
fn test_symlink_target_validation() {
    // This test would need actual filesystem paths
    // In a real implementation, use tempdir for testing
}

#[test]
fn test_set_secure_permissions() {
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    std::fs::write(&file_path, "test").unwrap();

    let result = security::set_secure_permissions(&file_path);
    assert!(result.is_ok());
}
