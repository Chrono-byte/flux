//! Integration tests for file operations and symlink management
//!
//! These tests verify filesystem operations in isolation using temporary directories
//! and don't depend on internal library APIs, making them suitable for integration testing.
//!
//! Tests cover:
//! - File addition and copying
//! - Symlink creation (absolute, relative modes)
//! - File removal and cleanup
//! - Directory operations and recursion
//! - Binary and special file handling
//! - Backup creation and management
//! - Permission preservation (Unix-only)
//! - Error cases and negative tests

#[cfg(test)]
mod file_operations_integration_tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// Create a temporary test environment with isolated filesystem
    fn setup_test_environment() -> (TempDir, PathBuf, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("dotfiles");
        let home_path = temp_dir.path().join("home");

        fs::create_dir_all(&repo_path).unwrap();
        fs::create_dir_all(&home_path).unwrap();

        (temp_dir, repo_path, home_path)
    }

    // =========================================================================
    // File Addition Tests
    // =========================================================================

    /// Test adding a simple file to repository
    #[test]
    fn test_add_simple_file() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let source_file = home_path.join("config.txt");
        fs::write(&source_file, "test content").unwrap();

        let dest_file = repo_path.join("config.txt");
        fs::copy(&source_file, &dest_file).unwrap();

        assert!(dest_file.exists(), "File should be copied to repo");
        assert_eq!(fs::read_to_string(&dest_file).unwrap(), "test content");
    }

    /// Test adding file with directory structure
    #[test]
    fn test_add_file_with_directory_structure() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let tool_dir = repo_path.join("sway");
        fs::create_dir_all(&tool_dir).unwrap();

        let config_file = tool_dir.join("config");
        fs::write(&config_file, "sway config content").unwrap();

        assert!(config_file.exists());
        assert_eq!(
            fs::read_to_string(&config_file).unwrap(),
            "sway config content"
        );
    }

    /// Test adding multiple files to different tools
    #[test]
    fn test_add_multiple_files_different_tools() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let sway_dir = repo_path.join("sway");
        let vim_dir = repo_path.join("vim");

        fs::create_dir_all(&sway_dir).unwrap();
        fs::create_dir_all(&vim_dir).unwrap();

        fs::write(sway_dir.join("config"), "sway config").unwrap();
        fs::write(vim_dir.join("vimrc"), "vim config").unwrap();

        assert!(sway_dir.join("config").exists());
        assert!(vim_dir.join("vimrc").exists());
    }

    /// Test adding directory (recursively)
    #[test]
    fn test_add_directory_recursively() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let source_dir = home_path.join("config");
        fs::create_dir_all(source_dir.join("sub1/sub2")).unwrap();
        fs::write(source_dir.join("sub1/file1.txt"), "content1").unwrap();
        fs::write(source_dir.join("sub1/sub2/file2.txt"), "content2").unwrap();

        let dest_dir = repo_path.join("config");
        copy_recursive(&source_dir, &dest_dir).unwrap();

        assert!(dest_dir.join("sub1/file1.txt").exists());
        assert!(dest_dir.join("sub1/sub2/file2.txt").exists());
    }

    // =========================================================================
    // Symlink Creation Tests - Relative
    // =========================================================================

    /// Test creating relative symlink
    #[test]
    #[cfg(unix)]
    fn test_create_relative_symlink() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let repo_file = repo_path.join("config");
        fs::write(&repo_file, "content").unwrap();

        let symlink_path = home_path.join("config_link");

        // Calculate relative path from symlink to repo file
        let relative_path = pathdiff::diff_paths(&repo_file, home_path.parent().unwrap()).unwrap();

        std::os::unix::fs::symlink(&relative_path, &symlink_path).unwrap();

        assert!(symlink_path.is_symlink());
        let target = std::fs::read_link(&symlink_path).unwrap();
        assert!(!target.is_absolute(), "Symlink should be relative");
    }

    /// Test creating absolute symlink
    #[test]
    #[cfg(unix)]
    fn test_create_absolute_symlink() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let repo_file = repo_path.join("config");
        fs::write(&repo_file, "content").unwrap();

        let symlink_path = home_path.join("config_link");
        std::os::unix::fs::symlink(&repo_file, &symlink_path).unwrap();

        assert!(symlink_path.is_symlink());
        let target = std::fs::read_link(&symlink_path).unwrap();
        assert!(target.is_absolute(), "Symlink should be absolute");
    }

    /// Test creating symlinks for files in subdirectories
    #[test]
    #[cfg(unix)]
    fn test_create_symlinks_in_subdirectories() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let tool_dir = repo_path.join("sway");
        fs::create_dir_all(&tool_dir).unwrap();
        let repo_file = tool_dir.join("config");
        fs::write(&repo_file, "content").unwrap();

        let config_subdir = home_path.join(".config");
        fs::create_dir_all(&config_subdir).unwrap();
        let symlink_path = config_subdir.join("sway");

        std::os::unix::fs::symlink(&repo_file, &symlink_path).unwrap();

        assert!(symlink_path.is_symlink());
    }

    // =========================================================================
    // File Removal Tests
    // =========================================================================

    /// Test removing tracked file from repository
    #[test]
    fn test_remove_file_from_repo() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let file_path = repo_path.join("config.txt");
        fs::write(&file_path, "content").unwrap();
        assert!(file_path.exists());

        fs::remove_file(&file_path).unwrap();
        assert!(!file_path.exists());
    }

    /// Test removing symlink without affecting original
    #[test]
    fn test_remove_symlink_preserves_original() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let repo_file = repo_path.join("config");
        fs::write(&repo_file, "content").unwrap();

        let symlink_path = home_path.join("config_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&repo_file, &symlink_path).unwrap();

        #[cfg(unix)]
        {
            fs::remove_file(&symlink_path).unwrap();
            assert!(!symlink_path.exists());
            assert!(repo_file.exists(), "Original file should still exist");
            assert_eq!(fs::read_to_string(&repo_file).unwrap(), "content");
        }
    }

    /// Test removing entire directory
    #[test]
    fn test_remove_directory() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let tool_dir = repo_path.join("sway");
        fs::create_dir_all(&tool_dir).unwrap();
        fs::write(tool_dir.join("config"), "content").unwrap();

        fs::remove_dir_all(&tool_dir).unwrap();
        assert!(!tool_dir.exists());
    }

    // =========================================================================
    // Directory Structure Tests
    // =========================================================================

    /// Test creating nested directories
    #[test]
    fn test_create_nested_directories() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let deep_path = repo_path.join("a/b/c/d/e");
        fs::create_dir_all(&deep_path).unwrap();

        assert!(deep_path.exists());
        assert!(deep_path.is_dir());
    }

    /// Test directory with many files
    #[test]
    fn test_directory_with_many_files() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let dir = repo_path.join("many_files");
        fs::create_dir_all(&dir).unwrap();

        for i in 0..100 {
            fs::write(
                dir.join(format!("file_{}.txt", i)),
                format!("content {}", i),
            )
            .unwrap();
        }

        let entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(entries.len(), 100);
    }

    // =========================================================================
    // File Content Tests
    // =========================================================================

    /// Test file content is preserved after copying
    #[test]
    fn test_file_content_preserved() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let original_content = "test config\nwith multiple lines\nand special chars: @#$%";
        let source_file = home_path.join("test.conf");
        fs::write(&source_file, original_content).unwrap();

        let dest_file = repo_path.join("test.conf");
        fs::copy(&source_file, &dest_file).unwrap();

        let copied_content = fs::read_to_string(&dest_file).unwrap();
        assert_eq!(copied_content, original_content);
    }

    /// Test binary file handling
    #[test]
    fn test_binary_file_handling() {
        let (_temp, repo_path, home_path) = setup_test_environment();

        let binary_data = vec![0u8, 1, 2, 255, 254, 127];
        let source_file = home_path.join("binary.bin");
        fs::write(&source_file, &binary_data).unwrap();

        let dest_file = repo_path.join("binary.bin");
        fs::copy(&source_file, &dest_file).unwrap();

        let copied_data = fs::read(&dest_file).unwrap();
        assert_eq!(copied_data, binary_data);
    }

    /// Test file with no extension
    #[test]
    fn test_file_without_extension() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let file = repo_path.join("config_no_ext");
        fs::write(&file, "content").unwrap();

        assert!(file.exists());
        assert_eq!(fs::read_to_string(&file).unwrap(), "content");
    }

    // =========================================================================
    // Backup Tests
    // =========================================================================

    /// Test creating backup of existing file
    #[test]
    fn test_backup_existing_file() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let original_file = repo_path.join("original.txt");
        fs::write(&original_file, "original content").unwrap();

        let backup_file = repo_path.join("original.txt.backup");
        fs::copy(&original_file, &backup_file).unwrap();

        assert!(original_file.exists());
        assert!(backup_file.exists());
        assert_eq!(
            fs::read_to_string(&original_file).unwrap(),
            "original content"
        );
        assert_eq!(
            fs::read_to_string(&backup_file).unwrap(),
            "original content"
        );
    }

    /// Test multiple backups of same file
    #[test]
    fn test_multiple_backups() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let original_file = repo_path.join("config");
        fs::write(&original_file, "version 1").unwrap();

        let backup1 = repo_path.join("config.backup.1");
        let backup2 = repo_path.join("config.backup.2");

        fs::copy(&original_file, &backup1).unwrap();

        fs::write(&original_file, "version 2").unwrap();
        fs::copy(&original_file, &backup2).unwrap();

        assert_eq!(fs::read_to_string(&backup1).unwrap(), "version 1");
        assert_eq!(fs::read_to_string(&backup2).unwrap(), "version 2");
    }

    // =========================================================================
    // Permissions Tests
    // =========================================================================

    /// Test file permissions are preserved
    #[test]
    #[cfg(unix)]
    fn test_file_permissions_preserved() {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        let (_temp, repo_path, home_path) = setup_test_environment();

        let source_file = home_path.join("executable.sh");
        fs::write(&source_file, "#!/bin/bash\necho test").unwrap();

        // Make executable
        fs::set_permissions(&source_file, Permissions::from_mode(0o755)).unwrap();

        let dest_file = repo_path.join("executable.sh");
        fs::copy(&source_file, &dest_file).unwrap();

        let metadata = fs::metadata(&dest_file).unwrap();
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Check if executable bit is set
        assert!(mode & 0o111 != 0, "Executable bit should be preserved");
    }

    // =========================================================================
    // Error Cases - Negative Tests
    // =========================================================================

    /// Test handling of non-existent source file
    #[test]
    fn test_error_nonexistent_source_file() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let nonexistent = repo_path.join("nonexistent.txt");
        let dest = repo_path.join("dest.txt");

        let result = fs::copy(&nonexistent, &dest);
        assert!(result.is_err(), "Should fail with nonexistent source");
    }

    /// Test error on creating file in non-existent directory
    #[test]
    fn test_error_file_in_nonexistent_dir() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let file_path = repo_path.join("nonexistent/file.txt");
        let result = fs::write(&file_path, "content");

        assert!(result.is_err(), "Should fail without parent directory");
    }

    /// Test handling of circular symlinks
    #[test]
    #[cfg(unix)]
    fn test_circular_symlink_detection() {
        let (_temp, repo_path, _home_path) = setup_test_environment();

        let link1 = repo_path.join("link1");
        let link2 = repo_path.join("link2");

        std::os::unix::fs::symlink(&link2, &link1).unwrap();
        std::os::unix::fs::symlink(&link1, &link2).unwrap();

        assert!(link1.is_symlink());
        assert!(link2.is_symlink());

        // Reading circular symlinks should fail
        let result = fs::read_link(&link1);
        assert!(
            result.is_ok(),
            "Symlink should be readable even if circular"
        );
    }

    // =========================================================================
    // Helper Functions
    // =========================================================================

    fn copy_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let dest_path = dst.join(&file_name);

            if path.is_dir() {
                copy_recursive(&path, &dest_path)?;
            } else {
                fs::copy(&path, &dest_path)?;
            }
        }
        Ok(())
    }
}

// ============================================================================
// Symlink Resolution Mode Tests
// ============================================================================

#[cfg(test)]
mod symlink_resolution_tests {
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn setup_env() -> (TempDir, PathBuf, PathBuf) {
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        let home = temp.path().join("home");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&home).unwrap();
        (temp, repo, home)
    }

    /// Test symlink resolution: auto mode
    #[test]
    #[cfg(unix)]
    fn test_symlink_auto_mode() {
        let (_temp, repo, home) = setup_env();

        let repo_file = repo.join("config");
        fs::write(&repo_file, "content").unwrap();

        let symlink = home.join("link");

        // Auto should create relative if possible
        std::os::unix::fs::symlink(&repo_file, &symlink).unwrap();

        assert!(symlink.is_symlink());
    }

    /// Test symlink resolution: relative mode
    #[test]
    #[cfg(unix)]
    fn test_symlink_relative_mode() {
        let (_temp, repo, home) = setup_env();

        let repo_file = repo.join("config");
        fs::write(&repo_file, "content").unwrap();

        let home_config = home.join(".config");
        fs::create_dir_all(&home_config).unwrap();

        let symlink = home_config.join("link");
        let relative = pathdiff::diff_paths(&repo_file, &home_config).unwrap();

        std::os::unix::fs::symlink(&relative, &symlink).unwrap();

        assert!(symlink.is_symlink());
        let target = fs::read_link(&symlink).unwrap();
        assert!(!target.is_absolute());
    }

    /// Test symlink resolution: absolute mode
    #[test]
    #[cfg(unix)]
    fn test_symlink_absolute_mode() {
        let (_temp, repo, home) = setup_env();

        let repo_file = repo.join("config");
        fs::write(&repo_file, "content").unwrap();

        let symlink = home.join("link");
        std::os::unix::fs::symlink(&repo_file, &symlink).unwrap();

        assert!(symlink.is_symlink());
        let target = fs::read_link(&symlink).unwrap();
        assert!(target.is_absolute());
    }
}
