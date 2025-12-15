//! Tests for error utility functions

use flux::utils::error_utils::{ErrorBuilder, home_dir_not_found};

#[test]
fn test_error_builder_basic() {
    let err = ErrorBuilder::new("File not found")
        .why("Required configuration is missing")
        .solution("Check the file path")
        .context("Path", "/path/to/file")
        .build_path_error();

    let msg = err.to_string();
    assert!(msg.contains("What: File not found"));
    assert!(msg.contains("Why: Required configuration is missing"));
    assert!(msg.contains("Solution"));
    assert!(msg.contains("Path: /path/to/file"));
}

#[test]
fn test_home_dir_not_found_message() {
    let err = home_dir_not_found();
    let msg = err.to_string();
    assert!(msg.contains("$HOME"));
    assert!(msg.contains("Solution"));
}
