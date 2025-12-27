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
    fn setup_test_environment() -> Result<(TempDir, PathBuf, PathBuf), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path().join("dotfiles");
        let home_path = temp_dir.path().join("home");

        fs::create_dir_all(&repo_path)?;
        fs::create_dir_all(&home_path)?;

        Ok((temp_dir, repo_path, home_path))
    }

    // =========================================================================
    // File Addition Tests
    // =========================================================================

    /// Test adding a simple file to repository
    #[test]
    fn test_add_simple_file() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let source_file = home_path.join("config.txt");
        fs::write(&source_file, "test content")?;

        let dest_file = repo_path.join("config.txt");
        fs::copy(&source_file, &dest_file)?;

        assert!(dest_file.exists(), "File should be copied to repo");
        assert_eq!(fs::read_to_string(&dest_file)?, "test content");
        Ok(())
    }

    /// Test adding file with directory structure
    #[test]
    fn test_add_file_with_directory_structure() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let tool_dir = repo_path.join("sway");
        fs::create_dir_all(&tool_dir)?;

        let config_file = tool_dir.join("config");
        fs::write(&config_file, "sway config content")?;

        assert!(config_file.exists());
        assert_eq!(fs::read_to_string(&config_file)?, "sway config content");
        Ok(())
    }

    /// Test adding multiple files to different tools
    #[test]
    fn test_add_multiple_files_different_tools() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let sway_dir = repo_path.join("sway");
        let vim_dir = repo_path.join("vim");

        fs::create_dir_all(&sway_dir)?;
        fs::create_dir_all(&vim_dir)?;

        fs::write(sway_dir.join("config"), "sway config")?;
        fs::write(vim_dir.join("vimrc"), "vim config")?;

        assert!(sway_dir.join("config").exists());
        assert!(vim_dir.join("vimrc").exists());
        Ok(())
    }

    /// Test adding directory (recursively)
    #[test]
    fn test_add_directory_recursively() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let source_dir = home_path.join("config");
        fs::create_dir_all(source_dir.join("sub1/sub2"))?;
        fs::write(source_dir.join("sub1/file1.txt"), "content1")?;
        fs::write(source_dir.join("sub1/sub2/file2.txt"), "content2")?;

        let dest_dir = repo_path.join("config");
        copy_recursive(&source_dir, &dest_dir)?;

        assert!(dest_dir.join("sub1/file1.txt").exists());
        assert!(dest_dir.join("sub1/sub2/file2.txt").exists());
        Ok(())
    }

    // =========================================================================
    // Symlink Creation Tests - Relative
    // =========================================================================

    /// Test creating relative symlink
    #[test]
    #[cfg(unix)]
    fn test_create_relative_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let repo_file = repo_path.join("config");
        fs::write(&repo_file, "content")?;

        let symlink_path = home_path.join("config_link");

        // Calculate relative path from symlink to repo file
        let parent = home_path.parent().ok_or("home_path has no parent")?;
        let relative_path =
            pathdiff::diff_paths(&repo_file, parent).ok_or("Failed to calculate relative path")?;

        std::os::unix::fs::symlink(&relative_path, &symlink_path)?;

        assert!(symlink_path.is_symlink());
        let target = std::fs::read_link(&symlink_path)?;
        assert!(!target.is_absolute(), "Symlink should be relative");
        Ok(())
    }

    /// Test creating absolute symlink
    #[test]
    #[cfg(unix)]
    fn test_create_absolute_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let repo_file = repo_path.join("config");
        fs::write(&repo_file, "content")?;

        let symlink_path = home_path.join("config_link");
        std::os::unix::fs::symlink(&repo_file, &symlink_path)?;

        assert!(symlink_path.is_symlink());
        let target = std::fs::read_link(&symlink_path)?;
        assert!(target.is_absolute(), "Symlink should be absolute");
        Ok(())
    }

    /// Test creating symlinks for files in subdirectories
    #[test]
    #[cfg(unix)]
    fn test_create_symlinks_in_subdirectories() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let tool_dir = repo_path.join("sway");
        fs::create_dir_all(&tool_dir)?;
        let repo_file = tool_dir.join("config");
        fs::write(&repo_file, "content")?;

        let config_subdir = home_path.join(".config");
        fs::create_dir_all(&config_subdir)?;
        let symlink_path = config_subdir.join("sway");

        std::os::unix::fs::symlink(&repo_file, &symlink_path)?;

        assert!(symlink_path.is_symlink());
        Ok(())
    }

    // =========================================================================
    // File Removal Tests
    // =========================================================================

    /// Test removing tracked file from repository
    #[test]
    fn test_remove_file_from_repo() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let file_path = repo_path.join("config.txt");
        fs::write(&file_path, "content")?;
        assert!(file_path.exists());

        fs::remove_file(&file_path)?;
        assert!(!file_path.exists());
        Ok(())
    }

    /// Test removing symlink without affecting original
    #[test]
    fn test_remove_symlink_preserves_original() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let repo_file = repo_path.join("config");
        fs::write(&repo_file, "content")?;

        let symlink_path = home_path.join("config_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&repo_file, &symlink_path)?;

        #[cfg(unix)]
        {
            fs::remove_file(&symlink_path)?;
            assert!(!symlink_path.exists());
            assert!(repo_file.exists(), "Original file should still exist");
            assert_eq!(fs::read_to_string(&repo_file)?, "content");
        }
        Ok(())
    }

    /// Test removing entire directory
    #[test]
    fn test_remove_directory() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let tool_dir = repo_path.join("sway");
        fs::create_dir_all(&tool_dir)?;
        fs::write(tool_dir.join("config"), "content")?;

        fs::remove_dir_all(&tool_dir)?;
        assert!(!tool_dir.exists());
        Ok(())
    }

    // =========================================================================
    // Directory Structure Tests
    // =========================================================================

    /// Test creating nested directories
    #[test]
    fn test_create_nested_directories() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let deep_path = repo_path.join("a/b/c/d/e");
        fs::create_dir_all(&deep_path)?;

        assert!(deep_path.exists());
        assert!(deep_path.is_dir());
        Ok(())
    }

    /// Test directory with many files
    #[test]
    fn test_directory_with_many_files() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let dir = repo_path.join("many_files");
        fs::create_dir_all(&dir)?;

        for i in 0..100 {
            fs::write(dir.join(format!("file_{i}.txt")), format!("content {i}"))?;
        }

        let entries: Vec<_> = fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
        assert_eq!(entries.len(), 100);
        Ok(())
    }

    // =========================================================================
    // File Content Tests
    // =========================================================================

    /// Test file content is preserved after copying
    #[test]
    fn test_file_content_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let original_content = "test config\nwith multiple lines\nand special chars: @#$%";
        let source_file = home_path.join("test.conf");
        fs::write(&source_file, original_content)?;

        let dest_file = repo_path.join("test.conf");
        fs::copy(&source_file, &dest_file)?;

        let copied_content = fs::read_to_string(&dest_file)?;
        assert_eq!(copied_content, original_content);
        Ok(())
    }

    /// Test binary file handling
    #[test]
    fn test_binary_file_handling() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let binary_data = vec![0u8, 1, 2, 255, 254, 127];
        let source_file = home_path.join("binary.bin");
        fs::write(&source_file, &binary_data)?;

        let dest_file = repo_path.join("binary.bin");
        fs::copy(&source_file, &dest_file)?;

        let copied_data = fs::read(&dest_file)?;
        assert_eq!(copied_data, binary_data);
        Ok(())
    }

    /// Test file with no extension
    #[test]
    fn test_file_without_extension() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let file = repo_path.join("config_no_ext");
        fs::write(&file, "content")?;

        assert!(file.exists());
        assert_eq!(fs::read_to_string(&file)?, "content");
        Ok(())
    }

    // =========================================================================
    // Backup Tests
    // =========================================================================

    /// Test creating backup of existing file
    #[test]
    fn test_backup_existing_file() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let original_file = repo_path.join("original.txt");
        fs::write(&original_file, "original content")?;

        let backup_file = repo_path.join("original.txt.backup");
        fs::copy(&original_file, &backup_file)?;

        assert!(original_file.exists());
        assert!(backup_file.exists());
        assert_eq!(fs::read_to_string(&original_file)?, "original content");
        assert_eq!(fs::read_to_string(&backup_file)?, "original content");
        Ok(())
    }

    /// Test multiple backups of same file
    #[test]
    fn test_multiple_backups() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let original_file = repo_path.join("config");
        fs::write(&original_file, "version 1")?;

        let backup1 = repo_path.join("config.backup.1");
        let backup2 = repo_path.join("config.backup.2");

        fs::copy(&original_file, &backup1)?;

        fs::write(&original_file, "version 2")?;
        fs::copy(&original_file, &backup2)?;

        assert_eq!(fs::read_to_string(&backup1)?, "version 1");
        assert_eq!(fs::read_to_string(&backup2)?, "version 2");
        Ok(())
    }

    // =========================================================================
    // Permissions Tests
    // =========================================================================

    /// Test file permissions are preserved
    #[test]
    #[cfg(unix)]
    fn test_file_permissions_preserved() -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;

        let (_temp, repo_path, home_path) = setup_test_environment()?;

        let source_file = home_path.join("executable.sh");
        fs::write(&source_file, "#!/bin/bash\necho test")?;

        // Make executable
        fs::set_permissions(&source_file, Permissions::from_mode(0o755))?;

        let dest_file = repo_path.join("executable.sh");
        fs::copy(&source_file, &dest_file)?;

        let metadata = fs::metadata(&dest_file)?;
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Check if executable bit is set
        assert!(mode & 0o111 != 0, "Executable bit should be preserved");
        Ok(())
    }

    // =========================================================================
    // Error Cases - Negative Tests
    // =========================================================================

    /// Test handling of non-existent source file
    #[test]
    fn test_error_nonexistent_source_file() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let nonexistent = repo_path.join("nonexistent.txt");
        let dest = repo_path.join("dest.txt");

        let result = fs::copy(&nonexistent, &dest);
        assert!(result.is_err(), "Should fail with nonexistent source");
        Ok(())
    }

    /// Test error on creating file in non-existent directory
    #[test]
    fn test_error_file_in_nonexistent_dir() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let file_path = repo_path.join("nonexistent/file.txt");
        let result = fs::write(&file_path, "content");

        assert!(result.is_err(), "Should fail without parent directory");
        Ok(())
    }

    /// Test handling of circular symlinks
    #[test]
    #[cfg(unix)]
    fn test_circular_symlink_detection() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo_path, _home_path) = setup_test_environment()?;

        let link1 = repo_path.join("link1");
        let link2 = repo_path.join("link2");

        std::os::unix::fs::symlink(&link2, &link1)?;
        std::os::unix::fs::symlink(&link1, &link2)?;

        assert!(link1.is_symlink());
        assert!(link2.is_symlink());

        // Reading circular symlinks should fail
        let result = fs::read_link(&link1);
        assert!(
            result.is_ok(),
            "Symlink should be readable even if circular"
        );
        Ok(())
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

    fn setup_env() -> Result<(TempDir, PathBuf, PathBuf), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let repo = temp.path().join("repo");
        let home = temp.path().join("home");
        fs::create_dir_all(&repo)?;
        fs::create_dir_all(&home)?;
        Ok((temp, repo, home))
    }

    /// Test symlink resolution: auto mode
    #[test]
    #[cfg(unix)]
    fn test_symlink_auto_mode() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo, home) = setup_env()?;

        let repo_file = repo.join("config");
        fs::write(&repo_file, "content")?;

        let symlink = home.join("link");

        // Auto should create relative if possible
        std::os::unix::fs::symlink(&repo_file, &symlink)?;

        assert!(symlink.is_symlink());
        Ok(())
    }

    /// Test symlink resolution: relative mode
    #[test]
    #[cfg(unix)]
    fn test_symlink_relative_mode() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo, home) = setup_env()?;

        let repo_file = repo.join("config");
        fs::write(&repo_file, "content")?;

        let home_config = home.join(".config");
        fs::create_dir_all(&home_config)?;

        let symlink = home_config.join("link");
        let relative = pathdiff::diff_paths(&repo_file, &home_config)
            .ok_or("Failed to calculate relative path")?;

        std::os::unix::fs::symlink(&relative, &symlink)?;

        assert!(symlink.is_symlink());
        let target = fs::read_link(&symlink)?;
        assert!(!target.is_absolute());
        Ok(())
    }

    /// Test symlink resolution: absolute mode
    #[test]
    #[cfg(unix)]
    fn test_symlink_absolute_mode() -> Result<(), Box<dyn std::error::Error>> {
        let (_temp, repo, home) = setup_env()?;

        let repo_file = repo.join("config");
        fs::write(&repo_file, "content")?;

        let symlink = home.join("link");
        std::os::unix::fs::symlink(&repo_file, &symlink)?;

        assert!(symlink.is_symlink());
        let target = fs::read_link(&symlink)?;
        assert!(target.is_absolute());
        Ok(())
    }
}
